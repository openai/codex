from __future__ import annotations

import time
from dataclasses import dataclass
from typing import Any, Dict, Optional, List

from .config import CONFIG
from .utils import log
from . import db
from .db import record_oms_event


@dataclass
class OMSOrder:
    client_oid: str
    symbol: str
    venue: str
    side: str
    order_type: str
    qty: float
    price: float
    sleeve: str
    status: str = "NEW"  # NEW, OPEN, PARTIAL, FILLED, CANCELED, REJECTED
    exchange_id: str = ""


def make_client_oid(prefix: str, child: int) -> str:
    """Create a deterministic, exchange-safe client order id.

    Many venues cap clientOrderId length (often 32-64). We keep it short and stable.
    """
    return f"{prefix}-{child}"[:48]


def record_new(o: OMSOrder, meta: Optional[Dict[str, Any]] = None) -> None:
    db.upsert_order(
        client_oid=o.client_oid,
        venue=o.venue,
        sym=o.symbol,
        side=o.side,
        type_=o.order_type,
        status=o.status,
        price=o.price,
        qty=o.qty,
        filled=0.0,
        remaining=o.qty,
        exchange_order_id=o.exchange_id,
        sleeve=o.sleeve,
        reason="",
        meta=meta or {},
    )


def update_status(client_oid: str, status: str, exchange_id: str = "", filled: float | None = None, remaining: float | None = None, meta: Optional[Dict[str, Any]] = None) -> None:
    # Store primary fields
    db.update_order_status(client_oid, status, filled=filled, remaining=remaining, exchange_order_id=exchange_id or None)
    # If we have extra meta, fold it into the order row via upsert (keeps last meta)
    if meta is not None:
        # Read existing order and re-upsert meta (best-effort)
        try:
            rows = [r for r in db.get_open_orders() if str(r.get("client_oid")) == str(client_oid)]
            if rows:
                r = rows[0]
                db.upsert_order(
                    client_oid=client_oid,
                    venue=str(r.get("venue")),
                    sym=str(r.get("sym")),
                    side=str(r.get("side")),
                    type_=str(r.get("type")),
                    status=str(status),
                    price=float(r.get("price") or 0.0),
                    qty=float(r.get("qty") or 0.0),
                    filled=float(r.get("filled") or 0.0),
                    remaining=float(r.get("remaining") or 0.0),
                    exchange_order_id=exchange_id or str(r.get("exchange_order_id") or ""),
                    sleeve=str(r.get("sleeve") or ""),
                    reason=str(r.get("reason") or ""),
                    meta=meta,
                )
        except Exception:
            pass


async def cancel_order(broker, sym: str, venue: str, exchange_id: str) -> bool:
    try:
        return await broker.cancel_order(exchange_id, sym, venue)
    except Exception:
        return False


async def cancel_replace_with_status(
    broker,
    *,
    sym: str,
    venue: str,
    prefix: str,
    sleeve: str,
) -> int:
    """Idempotent cancel/replace that checks exchange order status.

    Behavior:
      - Look up open OMS orders for (sym,venue,sleeve) that match our prefix.
      - For each, fetch exchange order status. If already closed -> update DB.
      - Otherwise cancel and mark canceled.

    This is safer than blind cancel-open-orders because some exchanges may not surface
    clientOrderId cleanly; the DB becomes the source of truth.
    """
    mode = str(CONFIG.get("MODE", "shadow")).lower()
    if mode in ("paper", "shadow"):
        return 0

    canceled = 0
    try:
        open_rows = db.get_open_orders(sym=sym, venue=venue)
    except Exception:
        open_rows = []

    for r in open_rows:
        try:
            cid = str(r.get("client_oid") or "")
            if prefix and prefix not in cid:
                continue
            if sleeve and str(r.get("sleeve") or "") != str(sleeve):
                continue
            exid = str(r.get("exchange_order_id") or "")
            if not exid:
                # Can't query exchange; attempt a best-effort cancel by client_oid matching open orders list.
                continue

            # Check status on exchange
            ex = await broker.fetch_order(exid, sym, venue)
            st = str(ex.get("status") or "").upper()
            filled = float(ex.get("filled") or 0.0)
            remaining = float(ex.get("remaining") or 0.0)

            if st in ("CLOSED", "FILLED") or remaining <= 1e-12:
                update_status(cid, "FILLED", exchange_id=exid, filled=filled, remaining=0.0)
                record_oms_event(cid, venue, sym, "FILL", {"exchange_order_id": exid, "filled": float(filled)})
                continue
            if st in ("CANCELED", "CANCELLED"):
                update_status(cid, "CANCELED", exchange_id=exid, filled=filled, remaining=remaining)
                record_oms_event(cid, venue, sym, "CANCEL", {"exchange_order_id": exid, "filled": float(filled), "remaining": float(remaining)})
                continue

            # Attempt cancel
            ok = await cancel_order(broker, sym, venue, exid)
            if ok:
                canceled += 1
                update_status(cid, "CANCELED", exchange_id=exid, filled=filled, remaining=remaining)
                record_oms_event(cid, venue, sym, "CANCEL", {"exchange_order_id": exid, "filled": float(filled), "remaining": float(remaining)})
        except Exception:
            continue

    return canceled


async def reconcile(broker, *, symbols: List[str], venues: List[str], since_ms: int | None = None) -> None:
    """Prime OMS reconciliation.

    1) Reconcile open orders (state machine persistence) per venue/symbol.
    2) Reconcile trades (fills) and update sleeve-level PnL attribution.

    We store a last-reconciled timestamp in DB memory so restarts are restart-safe.
    """
    mode = str(CONFIG.get("MODE", "shadow")).lower()
    if mode in ("paper", "shadow"):
        return

    now_ms = int(time.time() * 1000)
    since_key = "oms_recon_since_ms"
    if since_ms is None:
        try:
            since_ms = int(db.recall_latest(since_key) or 0) or None
        except Exception:
            since_ms = None

    # 1) Reconcile open orders
    max_syms = int(CONFIG.get("RECON_SYMBOLS_LIMIT", 25))
    syms = symbols[:max_syms]
    for v in venues:
        for sym in syms:
            try:
                opens = await broker.fetch_open_orders(sym, v)
            except Exception:
                opens = []

            # Mark any DB-open orders not present on exchange as closed by querying their status
            try:
                db_open = db.get_open_orders(sym=sym, venue=v)
            except Exception:
                db_open = []

            open_ids = set(str(o.get("id") or "") for o in opens or [])
            for r in db_open:
                try:
                    cid = str(r.get("client_oid") or "")
                    exid = str(r.get("exchange_order_id") or "")
                    if exid and exid not in open_ids:
                        ex = await broker.fetch_order(exid, sym, v)
                        if ex:
                            st = str(ex.get("status") or "").upper()
                            filled = float(ex.get("filled") or 0.0)
                            remaining = float(ex.get("remaining") or 0.0)
                            if st in ("CLOSED", "FILLED") or remaining <= 1e-12:
                                update_status(cid, "FILLED", exchange_id=exid, filled=filled, remaining=0.0)
                            elif st in ("CANCELED", "CANCELLED"):
                                update_status(cid, "CANCELED", exchange_id=exid, filled=filled, remaining=remaining)
                except Exception:
                    continue

            # Upsert open orders we see
            for o in opens or []:
                try:
                    info = o.get("info") or {}
                    cid = str(o.get("clientOrderId") or o.get("clientOrderID") or info.get("clientOrderId") or info.get("client_order_id") or "")
                    exid = str(o.get("id") or "")
                    side = str(o.get("side") or "").upper()
                    typ = str(o.get("type") or "").upper()
                    amt = float(o.get("amount") or o.get("qty") or 0.0)
                    px = float(o.get("price") or 0.0)
                    filled = float(o.get("filled") or 0.0)
                    remaining = float(o.get("remaining") or max(0.0, amt - filled))
                    st = "PARTIAL" if filled > 1e-12 and remaining > 1e-12 else "OPEN"
                    sleeve = "unknown"
                    if cid and "|" in cid:
                        # baseid|sleeve-child
                        try:
                            sleeve = cid.split("|")[1].split("-")[0][:16]
                        except Exception:
                            sleeve = "unknown"
                    if cid:
                        db.upsert_order(
                            client_oid=cid,
                            venue=v,
                            sym=sym,
                            side=side,
                            type_=typ,
                            status=st,
                            price=px,
                            qty=amt,
                            filled=filled,
                            remaining=remaining,
                            exchange_order_id=exid,
                            sleeve=sleeve,
                            reason="recon",
                            meta={"source": "exchange_open_orders"},
                        )
                except Exception:
                    continue

    
    # 1b) Reconcile order history (closed/canceled) if supported.
    # This drives the OMS state machine from the exchange's order ledger, not just open orders.
    hist_limit = int(CONFIG.get("RECON_ORDERS_LIMIT", 200))
    for v in venues:
        for sym in syms:
            try:
                closed = await broker.fetch_closed_orders(sym, v, since_ms=since_ms, limit=hist_limit)
            except Exception:
                closed = []
            try:
                hist = await broker.fetch_orders(sym, v, since_ms=since_ms, limit=hist_limit)
            except Exception:
                hist = []
            # Merge, de-dup by exchange id
            seen = set()
            merged = []
            for o in (closed or []) + (hist or []):
                oid = str(o.get("id") or "")
                if oid and oid in seen:
                    continue
                if oid:
                    seen.add(oid)
                merged.append(o)

            for o in merged:
                try:
                    info = o.get("info") or {}
                    exid = str(o.get("id") or "")
                    cid = str(o.get("clientOrderId") or o.get("clientOrderID") or info.get("clientOrderId") or info.get("client_order_id") or "")
                    if not cid:
                        # We can still update any DB orders that have this exchange id
                        cid = db.find_client_oid_for_exchange_order(sym=sym, venue=v, exchange_order_id=exid) or ""
                    if not cid:
                        continue

                    st_raw = str(o.get("status") or info.get("status") or "").upper()
                    filled = float(o.get("filled") or info.get("filled") or 0.0)
                    amt = float(o.get("amount") or info.get("amount") or 0.0)
                    remaining = float(o.get("remaining") or info.get("remaining") or max(0.0, amt - filled))

                    if st_raw in ("CLOSED", "FILLED") or remaining <= 1e-12:
                        update_status(cid, "FILLED", exchange_id=exid, filled=filled, remaining=0.0)
                    elif st_raw in ("CANCELED", "CANCELLED"):
                        update_status(cid, "CANCELED", exchange_id=exid, filled=filled, remaining=remaining)
                    elif st_raw in ("REJECTED", "EXPIRED"):
                        update_status(cid, "REJECTED", exchange_id=exid, filled=filled, remaining=remaining)
                    elif filled > 1e-12 and remaining > 1e-12:
                        update_status(cid, "PARTIAL", exchange_id=exid, filled=filled, remaining=remaining)
                    else:
                        update_status(cid, "OPEN", exchange_id=exid, filled=filled, remaining=remaining)
                except Exception:
                    continue


# 2) Reconcile trades/fills and attribute to sleeves
    for v in venues:
        for sym in syms:
            try:
                trades = await broker.fetch_my_trades(sym, v, since_ms=since_ms, limit=int(CONFIG.get("RECON_TRADES_LIMIT", 200)))
            except Exception:
                trades = []

            for t in trades or []:
                try:
                    ts_ms = int(t.get("timestamp") or 0) if isinstance(t.get("timestamp"), (int, float)) else now_ms
                    ts = int(ts_ms / 1000)
                    side_raw = str(t.get("side") or "").upper()
                    side = "LONG" if side_raw in ("BUY", "LONG") else "SHORT"
                    qty = float(t.get("amount") or t.get("qty") or 0.0)
                    price = float(t.get("price") or 0.0)
                    if qty <= 0 or price <= 0:
                        continue

                    fee = 0.0
                    fee_ccy = ""
                    fee_obj = t.get("fee") or {}
                    if isinstance(fee_obj, dict):
                        fee = float(fee_obj.get("cost") or 0.0)
                        fee_ccy = str(fee_obj.get("currency") or "")

                    ex_order_id = str(t.get("order") or t.get("orderId") or "")
                    client_oid = str((t.get("info") or {}).get("clientOrderId") or (t.get("info") or {}).get("client_order_id") or "")

                    sleeve = "unknown"
                    if client_oid and "|" in client_oid:
                        try:
                            sleeve = client_oid.split("|")[1].split("-")[0][:16]
                        except Exception:
                            sleeve = "unknown"
                    if sleeve == "unknown" and ex_order_id:
                        sleeve = db.find_sleeve_for_exchange_order(sym=sym, venue=v, exchange_order_id=ex_order_id) or "unknown"

                    db.record_trade(sym, v, side, qty, price, sleeve=sleeve, reason="recon", meta={"raw": t}, fee=fee, fee_ccy=fee_ccy, order_id=ex_order_id, client_oid=client_oid, ts=ts)
                    db.apply_trade_to_sleeve(sym, sleeve, "BUY" if side == "LONG" else "SELL", qty, price, fee=fee)
                    if client_oid:
                        record_oms_event(client_oid, v, sym, "FILL", {"exchange_order_id": ex_order_id, "side": side, "qty": float(qty), "price": float(price), "fee": float(fee), "fee_ccy": fee_ccy, "sleeve": sleeve, "ts": int(ts_ms)})
                except Exception:
                    continue

    db.memorize(since_key, int(now_ms))

    # Optional: print sleeve pnl snapshot occasionally
    if bool(CONFIG.get("LOG_SLEEVE_PNL", False)):
        try:
            snap = db.sleeve_pnl_snapshot()
            if snap:
                top = sorted(snap, key=lambda r: abs(r.get("realized_pnl", 0.0)), reverse=True)[:10]
                log.info(f"[pnl] sleeve realized snapshot: {top}")
        except Exception:
            pass
