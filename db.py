from __future__ import annotations

import json
import sqlite3
import time
import hashlib
from typing import Any, Optional, List, Tuple, Dict

DB_PATH = "aether_edge.db"


def _conn() -> sqlite3.Connection:
    c = sqlite3.connect(DB_PATH, timeout=30)
    c.execute("PRAGMA journal_mode=WAL;")
    c.execute("PRAGMA synchronous=NORMAL;")

    # Generic memory KV
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS memory (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        k TEXT NOT NULL,
        v TEXT NOT NULL,
        ts INTEGER NOT NULL
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_memory_k_ts ON memory(k, ts)")

    # Candle cache for walk-forward / research harness
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS candles (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        sym TEXT NOT NULL,
        tf TEXT NOT NULL,
        ts INTEGER NOT NULL,
        o REAL, h REAL, l REAL, c REAL, v REAL
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_candles_sym_tf_ts ON candles(sym, tf, ts)")

    # Trade/fill ledger (source of truth for PnL attribution)
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS trades (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        sym TEXT NOT NULL,
        venue TEXT NOT NULL,
        side TEXT NOT NULL,
        qty REAL NOT NULL,
        price REAL NOT NULL,
        fee REAL DEFAULT 0,
        fee_ccy TEXT DEFAULT '',
        order_id TEXT DEFAULT '',
        client_oid TEXT DEFAULT '',
        sleeve TEXT DEFAULT '',
        reason TEXT DEFAULT '',
        meta TEXT DEFAULT '',
        ts INTEGER NOT NULL
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_trades_ts ON trades(ts)")
    c.execute("CREATE INDEX IF NOT EXISTS idx_trades_sym_ts ON trades(sym, ts)")

    # OMS order ledger (restart-safe state machine)
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS orders (
        client_oid TEXT PRIMARY KEY,
        venue TEXT NOT NULL,
        sym TEXT NOT NULL,
        side TEXT NOT NULL,
        type TEXT NOT NULL,
        status TEXT NOT NULL,
        price REAL,
        qty REAL NOT NULL,
        filled REAL DEFAULT 0,
        remaining REAL DEFAULT 0,
        exchange_order_id TEXT DEFAULT '',
        sleeve TEXT DEFAULT '',
        reason TEXT DEFAULT '',
        meta TEXT DEFAULT '',
        created_ts INTEGER NOT NULL,
        updated_ts INTEGER NOT NULL
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_orders_sym_venue_status ON orders(sym, venue, status)")
    c.execute("CREATE INDEX IF NOT EXISTS idx_orders_updated ON orders(updated_ts)")

    # FIX-style order lifecycle event log (fund-grade audit trail)
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS oms_events (
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        ts INTEGER NOT NULL,
        client_oid TEXT NOT NULL,
        venue TEXT NOT NULL,
        sym TEXT NOT NULL,
        event TEXT NOT NULL,
        data TEXT DEFAULT ''
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_oms_events_client_ts ON oms_events(client_oid, ts)")
    c.execute("CREATE INDEX IF NOT EXISTS idx_oms_events_ts ON oms_events(ts)")

    # v15: deduplicated OMS event stream.
    # We keep the original oms_events for backwards compatibility and add a
    # deduped table keyed by a stable event_key hash.
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS oms_events_v2 (
        event_key TEXT PRIMARY KEY,
        id INTEGER NOT NULL,
        ts INTEGER NOT NULL,
        client_oid TEXT NOT NULL,
        venue TEXT NOT NULL,
        sym TEXT NOT NULL,
        event TEXT NOT NULL,
        data TEXT DEFAULT ''
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_oms_events_v2_ts ON oms_events_v2(ts)")
    c.execute("CREATE INDEX IF NOT EXISTS idx_oms_events_v2_client_ts ON oms_events_v2(client_oid, ts)")

    # Idempotency ledger for child orders (kept for backward compatibility)
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS executed_orders (
        order_id TEXT PRIMARY KEY,
        sym TEXT NOT NULL,
        venue TEXT NOT NULL,
        side TEXT NOT NULL,
        qty REAL NOT NULL,
        price REAL NOT NULL,
        ts INTEGER NOT NULL
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_executed_orders_ts ON executed_orders(ts)")

    # Sleeve inventory for realized PnL attribution (avg cost method)
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS sleeve_positions (
        sleeve TEXT NOT NULL,
        sym TEXT NOT NULL,
        qty REAL NOT NULL,
        avg_cost REAL NOT NULL,
        realized_pnl REAL NOT NULL,
        updated_ts INTEGER NOT NULL,
        PRIMARY KEY (sleeve, sym)
      )
    """
    )

    # Sleeve mark-to-market snapshots (unrealized + exposure decomposition)
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS sleeve_mtm (
        ts INTEGER NOT NULL,
        sleeve TEXT NOT NULL,
        sym TEXT NOT NULL,
        qty REAL NOT NULL,
        avg_cost REAL NOT NULL,
        mark REAL NOT NULL,
        unrealized_pnl REAL NOT NULL,
        realized_pnl REAL NOT NULL,
        gross_notional REAL NOT NULL,
        net_notional REAL NOT NULL,
        PRIMARY KEY (ts, sleeve, sym)
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_sleeve_mtm_ts ON sleeve_mtm(ts)")

    # v15: execution analytics snapshots (per venue) for dashboarding and ops.
    c.execute(
        """
      CREATE TABLE IF NOT EXISTS exec_stats (
        ts INTEGER NOT NULL,
        venue TEXT NOT NULL,
        sym TEXT NOT NULL,
        maker_share REAL,
        reject_rate REAL,
        avg_slippage_bps REAL,
        fill_rate REAL,
        PRIMARY KEY (ts, venue, sym)
      )
    """
    )
    c.execute("CREATE INDEX IF NOT EXISTS idx_exec_stats_ts ON exec_stats(ts)")

    return c


# --------------------------- Memory helpers ---------------------------

def memorize(k: str, v: Any) -> None:
    c = _conn()
    try:
        c.execute("INSERT INTO memory(k,v,ts) VALUES(?,?,?)", (k, json.dumps(v, default=str), int(time.time())))
        c.commit()
    finally:
        c.close()


def recall_latest(k: str) -> Optional[Any]:
    c = _conn()
    try:
        row = c.execute("SELECT v FROM memory WHERE k=? ORDER BY ts DESC LIMIT 1", (k,)).fetchone()
        return json.loads(row[0]) if row else None
    finally:
        c.close()


# --------------------------- OMS event helpers ---------------------------

def count_oms_events(event: str, since_ts: Optional[int] = None) -> int:
    """Count OMS events from the deduplicated stream (oms_events_v2)."""
    c = _conn()
    try:
        if since_ts is None:
            row = c.execute(
                "SELECT COUNT(1) FROM oms_events_v2 WHERE event=?",
                (event,),
            ).fetchone()
        else:
            row = c.execute(
                "SELECT COUNT(1) FROM oms_events_v2 WHERE event=? AND ts>=?",
                (event, since_ts),
            ).fetchone()
        return int(row[0] or 0) if row else 0
    finally:
        c.close()


def count_blocked_orders(window_seconds: int = 24 * 3600) -> Dict[str, int]:
    """Convenience: count BLOCKED orders (total and recent window)."""
    now = int(time.time())
    since = now - int(window_seconds)
    return {
        "total": count_oms_events("BLOCKED", since_ts=None),
        "window": count_oms_events("BLOCKED", since_ts=since),
    }


def recall_recent(k: str, limit: int = 50) -> List[Any]:
    c = _conn()
    try:
        rows = c.execute("SELECT v FROM memory WHERE k=? ORDER BY ts DESC LIMIT ?", (k, int(limit))).fetchall()
        return [json.loads(r[0]) for r in rows]
    finally:
        c.close()


# --------------------------- Candle store ---------------------------

def store_candles(sym: str, tf: str, ohlcv: list) -> None:
    c = _conn()
    try:
        c.executemany(
            "INSERT INTO candles(sym,tf,ts,o,h,l,c,v) VALUES(?,?,?,?,?,?,?,?)",
            [
                (sym, tf, int(r[0] // 1000), float(r[1]), float(r[2]), float(r[3]), float(r[4]), float(r[5]))
                for r in ohlcv
            ],
        )
        c.commit()
    finally:
        c.close()


def load_candles(sym: str, tf: str, limit: int = 500) -> List[Tuple[int, float, float, float, float, float]]:
    c = _conn()
    try:
        rows = c.execute(
            "SELECT ts,o,h,l,c,v FROM candles WHERE sym=? AND tf=? ORDER BY ts DESC LIMIT ?",
            (sym, tf, int(limit)),
        ).fetchall()
        return list(reversed(rows))
    finally:
        c.close()


# --------------------------- OMS orders ---------------------------

def upsert_order(
    *,
    client_oid: str,
    venue: str,
    sym: str,
    side: str,
    type_: str,
    status: str,
    price: float | None,
    qty: float,
    filled: float = 0.0,
    remaining: float | None = None,
    exchange_order_id: str = "",
    sleeve: str = "",
    reason: str = "",
    meta: Any = None,
) -> None:
    ts = int(time.time())
    if remaining is None:
        remaining = max(0.0, float(qty) - float(filled))
    m = json.dumps(meta or {})
    with _conn() as c:
        # Insert or update
        c.execute(
            """
            INSERT INTO orders(client_oid,venue,sym,side,type,status,price,qty,filled,remaining,exchange_order_id,sleeve,reason,meta,created_ts,updated_ts)
            VALUES(?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?,?)
            ON CONFLICT(client_oid) DO UPDATE SET
              venue=excluded.venue,
              sym=excluded.sym,
              side=excluded.side,
              type=excluded.type,
              status=excluded.status,
              price=excluded.price,
              qty=excluded.qty,
              filled=excluded.filled,
              remaining=excluded.remaining,
              exchange_order_id=excluded.exchange_order_id,
              sleeve=excluded.sleeve,
              reason=excluded.reason,
              meta=excluded.meta,
              updated_ts=excluded.updated_ts
            """,
            (
                str(client_oid),
                str(venue),
                str(sym),
                str(side),
                str(type_),
                str(status),
                float(price) if price is not None else None,
                float(qty),
                float(filled),
                float(remaining),
                str(exchange_order_id or ""),
                str(sleeve or ""),
                str(reason or ""),
                m,
                ts,
                ts,
            ),
        )
        c.commit()


def get_open_orders(sym: str | None = None, venue: str | None = None) -> List[Dict[str, Any]]:
    q = "SELECT client_oid,venue,sym,side,type,status,price,qty,filled,remaining,exchange_order_id,sleeve,reason,meta,created_ts,updated_ts FROM orders WHERE status IN ('NEW','OPEN','PARTIAL')"
    args: list[Any] = []
    if sym is not None:
        q += " AND sym=?"
        args.append(str(sym))
    if venue is not None:
        q += " AND venue=?"
        args.append(str(venue))
    q += " ORDER BY updated_ts DESC"
    with _conn() as c:
        rows = c.execute(q, tuple(args)).fetchall()
    out: List[Dict[str, Any]] = []
    for r in rows:
        out.append(
            {
                "client_oid": r[0],
                "venue": r[1],
                "sym": r[2],
                "side": r[3],
                "type": r[4],
                "status": r[5],
                "price": r[6],
                "qty": r[7],
                "filled": r[8],
                "remaining": r[9],
                "exchange_order_id": r[10],
                "sleeve": r[11],
                "reason": r[12],
                "meta": json.loads(r[13] or "{}"),
                "created_ts": r[14],
                "updated_ts": r[15],
            }
        )
    return out


def update_order_status(client_oid: str, status: str, filled: float | None = None, remaining: float | None = None, exchange_order_id: str | None = None) -> None:
    ts = int(time.time())
    sets = ["status=?", "updated_ts=?"]
    args: list[Any] = [str(status), ts]
    if filled is not None:
        sets.append("filled=?")
        args.append(float(filled))
    if remaining is not None:
        sets.append("remaining=?")
        args.append(float(remaining))
    if exchange_order_id is not None:
        sets.append("exchange_order_id=?")
        args.append(str(exchange_order_id))
    args.append(str(client_oid))
    with _conn() as c:
        c.execute(f"UPDATE orders SET {', '.join(sets)} WHERE client_oid=?", tuple(args))
        c.commit()


# --------------------------- Trade / Fill ledger ---------------------------

def record_trade(
    sym: str,
    venue: str,
    side: str,
    qty: float,
    price: float,
    sleeve: str = "",
    reason: str = "",
    meta: Any = None,
    fee: float = 0.0,
    fee_ccy: str = "",
    order_id: str = "",
    client_oid: str = "",
    ts: int | None = None,
) -> None:
    """Persist a trade/fill in the local SQLite ledger (best-effort)."""
    try:
        if ts is None:
            ts = int(time.time())
        m = json.dumps(meta or {})
        with _conn() as c:
            c.execute(
                "INSERT INTO trades(sym,venue,side,qty,price,fee,fee_ccy,order_id,client_oid,sleeve,reason,meta,ts) VALUES (?,?,?,?,?,?,?,?,?,?,?,?,?)",
                (
                    str(sym),
                    str(venue),
                    str(side),
                    float(qty),
                    float(price),
                    float(fee or 0.0),
                    str(fee_ccy or ""),
                    str(order_id or ""),
                    str(client_oid or ""),
                    str(sleeve or ""),
                    str(reason or ""),
                    m,
                    int(ts),
                ),
            )
            c.commit()
    except Exception:
        pass


def latest_trade_ts(venue: str, sym: str) -> int:
    try:
        with _conn() as c:
            row = c.execute("SELECT COALESCE(MAX(ts),0) FROM trades WHERE venue=? AND sym=?", (str(venue), str(sym))).fetchone()
            return int(row[0] or 0)
    except Exception:
        return 0


# --------------------------- OMS FIX-style events ---------------------------

def record_oms_event(client_oid: str, venue: str, sym: str, event: str, data: Dict[str, Any] | None = None, ts: int | None = None) -> None:
    """Persist an OMS event for auditability (FIX-like lifecycle)."""
    try:
        if ts is None:
            ts = int(time.time() * 1000)
        payload = json.dumps(data or {}, default=str)
        # v15: stable dedupe key. Include fields that uniquely identify the event.
        # We intentionally omit wall-clock ts so retries don't generate duplicates.
        event_key_src = f"{client_oid}|{venue}|{sym}|{event}|{payload}"
        event_key = hashlib.sha1(event_key_src.encode('utf-8')).hexdigest()
        with _conn() as c:
            c.execute(
                "INSERT INTO oms_events(ts,client_oid,venue,sym,event,data) VALUES(?,?,?,?,?,?)",
                (int(ts), str(client_oid), str(venue), str(sym), str(event), payload),
            )
            # Insert into deduped stream (id is the monotonic id from the primary table).
            try:
                last_id = c.execute("SELECT last_insert_rowid()").fetchone()[0]
                c.execute(
                    "INSERT OR IGNORE INTO oms_events_v2(event_key,id,ts,client_oid,venue,sym,event,data) VALUES(?,?,?,?,?,?,?,?)",
                    (event_key, int(last_id), int(ts), str(client_oid), str(venue), str(sym), str(event), payload),
                )
            except Exception:
                pass
            c.commit()
    except Exception:
        pass


# --------------------------- Idempotency (legacy child ledger) ---------------------------

def mark_executed_order(order_id: str, sym: str, venue: str, side: str, qty: float, price: float) -> bool:
    """Insert `order_id` if it doesn't exist.

    Returns True if inserted (i.e., not previously executed), False if already present.
    """
    try:
        ts = int(time.time())
        with _conn() as c:
            c.execute(
                "INSERT OR IGNORE INTO executed_orders(order_id,sym,venue,side,qty,price,ts) VALUES (?,?,?,?,?,?,?)",
                (str(order_id), str(sym), str(venue), str(side), float(qty), float(price), ts),
            )
            c.commit()
            return c.total_changes > 0
    except Exception:
        # fail closed
        return False


def has_executed_order(order_id: str) -> bool:
    try:
        with _conn() as c:
            row = c.execute("SELECT 1 FROM executed_orders WHERE order_id=? LIMIT 1", (str(order_id),)).fetchone()
            return bool(row)
    except Exception:
        return False


# --------------------------- Sleeve PnL attribution ---------------------------

def _get_sleeve_pos(c: sqlite3.Connection, sleeve: str, sym: str) -> tuple[float, float, float]:
    row = c.execute(
        "SELECT qty, avg_cost, realized_pnl FROM sleeve_positions WHERE sleeve=? AND sym=?",
        (str(sleeve), str(sym)),
    ).fetchone()
    if not row:
        return 0.0, 0.0, 0.0
    return float(row[0]), float(row[1]), float(row[2])


def _set_sleeve_pos(c: sqlite3.Connection, sleeve: str, sym: str, qty: float, avg_cost: float, realized: float) -> None:
    ts = int(time.time())
    c.execute(
        """
        INSERT INTO sleeve_positions(sleeve,sym,qty,avg_cost,realized_pnl,updated_ts)
        VALUES(?,?,?,?,?,?)
        ON CONFLICT(sleeve,sym) DO UPDATE SET
          qty=excluded.qty,
          avg_cost=excluded.avg_cost,
          realized_pnl=excluded.realized_pnl,
          updated_ts=excluded.updated_ts
        """,
        (str(sleeve), str(sym), float(qty), float(avg_cost), float(realized), ts),
    )


def apply_trade_to_sleeve(sym: str, sleeve: str, side: str, qty: float, price: float, fee: float = 0.0) -> None:
    """Update sleeve inventory and realized PnL using avg-cost method.

    This is *not* a full prime-broker accounting engine, but it is stable and auditable
    for sleeve-level attribution from fills.
    """
    sleeve = str(sleeve or "")
    if not sleeve:
        sleeve = "unknown"
    qty = float(qty)
    price = float(price)
    fee = float(fee or 0.0)
    sign = 1.0 if side.upper() in ("LONG", "BUY") else -1.0

    with _conn() as c:
        q0, avg0, pnl0 = _get_sleeve_pos(c, sleeve, sym)

        # signed position change in base units
        dq = sign * qty
        q1 = q0 + dq

        realized = pnl0
        avg = avg0

        if q0 == 0.0 or (q0 > 0 and dq > 0) or (q0 < 0 and dq < 0):
            # increasing same-direction inventory: update avg cost
            notional0 = abs(q0) * avg0
            notional1 = notional0 + abs(dq) * price
            avg = (notional1 / max(abs(q1), 1e-12)) if q1 != 0 else 0.0
        else:
            # reducing or flipping: realize PnL on the closed portion
            closed = min(abs(q0), abs(dq))
            # if q0 > 0 and we sell (dq negative): pnl = (sell - avg)*closed
            # if q0 < 0 and we buy (dq positive): pnl = (avg - buy)*closed
            if q0 > 0:
                realized += (price - avg0) * closed
            else:
                realized += (avg0 - price) * closed

            # If flipped, set new avg to trade price for remainder
            if abs(dq) > abs(q0):
                avg = price
            # If flat, avg resets
            if q1 == 0.0:
                avg = 0.0

        # Fees subtract from realized PnL
        realized -= fee

        _set_sleeve_pos(c, sleeve, sym, q1, avg, realized)
        c.commit()


def sleeve_pnl_snapshot() -> List[Dict[str, Any]]:
    with _conn() as c:
        rows = c.execute("SELECT sleeve,sym,qty,avg_cost,realized_pnl,updated_ts FROM sleeve_positions ORDER BY sleeve,sym").fetchall()
    out: List[Dict[str, Any]] = []
    for r in rows:
        out.append({"sleeve": r[0], "sym": r[1], "qty": float(r[2]), "avg_cost": float(r[3]), "realized_pnl": float(r[4]), "updated_ts": int(r[5])})
    return out


def find_sleeve_for_exchange_order(sym: str, venue: str, exchange_order_id: str) -> str:
    'Map an exchange order id to a sleeve using the orders table (best-effort).'
    if not exchange_order_id:
        return ""
    try:
        with _conn() as c:
            row = c.execute(
                "SELECT sleeve FROM orders WHERE exchange_order_id=? AND sym=? AND venue=? ORDER BY updated_ts DESC LIMIT 1",
                (str(exchange_order_id), str(sym), str(venue)),
            ).fetchone()
            return str(row[0]) if row and row[0] is not None else ""
    except Exception:
        return ""


def find_client_oid_for_exchange_order(sym: str, venue: str, exchange_order_id: str) -> str:
    """Map an exchange order id to our client_oid using the orders table (best-effort)."""
    if not exchange_order_id:
        return ""
    try:
        with _conn() as c:
            row = c.execute(
                "SELECT client_oid FROM orders WHERE exchange_order_id=? AND sym=? AND venue=? ORDER BY updated_ts DESC LIMIT 1",
                (str(exchange_order_id), str(sym), str(venue)),
            ).fetchone()
            return str(row[0]) if row and row[0] is not None else ""
    except Exception:
        return ""


def record_sleeve_mtm(ts: int, sleeve: str, sym: str, qty: float, avg_cost: float, mark: float, unrealized_pnl: float, realized_pnl: float) -> None:
    gross = abs(float(qty) * float(mark))
    net = float(qty) * float(mark)
    with _conn() as c:
        c.execute(
            "INSERT OR REPLACE INTO sleeve_mtm(ts,sleeve,sym,qty,avg_cost,mark,unrealized_pnl,realized_pnl,gross_notional,net_notional) VALUES(?,?,?,?,?,?,?,?,?,?)",
            (int(ts), str(sleeve), str(sym), float(qty), float(avg_cost), float(mark), float(unrealized_pnl), float(realized_pnl), float(gross), float(net)),
        )
        c.commit()


def latest_sleeve_mtm(limit: int = 200) -> List[Dict[str, Any]]:
    with _conn() as c:
        row = c.execute("SELECT MAX(ts) FROM sleeve_mtm").fetchone()
        if not row or row[0] is None:
            return []
        ts = int(row[0])
        rows = c.execute(
            "SELECT ts,sleeve,sym,qty,avg_cost,mark,unrealized_pnl,realized_pnl,gross_notional,net_notional FROM sleeve_mtm WHERE ts=? ORDER BY sleeve,sym LIMIT ?",
            (ts, int(limit)),
        ).fetchall()
    out = []
    for r in rows:
        out.append({
            "ts": int(r[0]),
            "sleeve": r[1],
            "sym": r[2],
            "qty": float(r[3]),
            "avg_cost": float(r[4]),
            "mark": float(r[5]),
            "unrealized_pnl": float(r[6]),
            "realized_pnl": float(r[7]),
            "gross_notional": float(r[8]),
            "net_notional": float(r[9]),
        })
    return out


# --------------------------- Execution analytics ---------------------------

def record_exec_stats(
    ts: int,
    venue: str,
    sym: str,
    maker_share: float | None = None,
    reject_rate: float | None = None,
    avg_slippage_bps: float | None = None,
    fill_rate: float | None = None,
) -> None:
    try:
        with _conn() as c:
            c.execute(
                "INSERT OR REPLACE INTO exec_stats(ts,venue,sym,maker_share,reject_rate,avg_slippage_bps,fill_rate) VALUES(?,?,?,?,?,?,?)",
                (int(ts), str(venue), str(sym), maker_share, reject_rate, avg_slippage_bps, fill_rate),
            )
            c.commit()
    except Exception:
        pass


def latest_exec_stats(limit: int = 500) -> List[Dict[str, Any]]:
    try:
        with _conn() as c:
            row = c.execute("SELECT MAX(ts) FROM exec_stats").fetchone()
            if not row or row[0] is None:
                return []
            ts = int(row[0])
            rows = c.execute(
                "SELECT ts,venue,sym,maker_share,reject_rate,avg_slippage_bps,fill_rate FROM exec_stats WHERE ts=? ORDER BY venue,sym LIMIT ?",
                (ts, int(limit)),
            ).fetchall()
        return [
            {
                "ts": int(r[0]),
                "venue": r[1],
                "sym": r[2],
                "maker_share": float(r[3]) if r[3] is not None else None,
                "reject_rate": float(r[4]) if r[4] is not None else None,
                "avg_slippage_bps": float(r[5]) if r[5] is not None else None,
                "fill_rate": float(r[6]) if r[6] is not None else None,
            }
            for r in rows
        ]
    except Exception:
        return []
