import streamlit as st
from aether_edge.utils import STATE
from aether_edge.db import recall_recent, latest_exec_stats, count_blocked_orders
from aether_edge.config import read_mode_request, write_mode_request, is_armed

st.set_page_config(page_title="AetherEdge", layout="wide")
st.title("🌀 AetherEdge")

# ---------------------- Mode / safety banner ----------------------
cur_mode = str(STATE.get("mode", "paper") or "paper").lower()
requested = read_mode_request() or cur_mode
armed = bool(is_armed())
blocked = count_blocked_orders(window_seconds=24 * 3600)

b1, b2, b3, b4 = st.columns(4)
b1.metric("Active mode", cur_mode)
b2.metric("Requested mode", requested)
b3.metric("ARMED", "yes" if armed else "no")
b4.metric("Orders BLOCKED (24h)", str(blocked.get("window", 0)), delta=f"total {blocked.get('total', 0)}")

if cur_mode in ("pilot", "live") and not armed:
    st.warning(
        "Pilot/Live selected but NOT ARMED. System will fail-closed and block all real orders until armed (console prompt / env token)."
    )

c1,c2,c3 = st.columns(3)
c1.metric("Equity", f"${STATE.get('equity',0.0):,.2f}")
c2.metric("Drawdown", f"{STATE.get('drawdown',0.0)*100:.2f}%")
c3.metric("Mode", str(STATE.get("mode","")))

st.divider()
st.subheader("Trade mode control")

cols = st.columns([2, 2, 4])
with cols[0]:
    req_mode = st.selectbox("Requested mode", ["paper", "pilot", "live"], index=["paper","pilot","live"].index(requested if requested in ("paper","pilot","live") else "paper"))
with cols[1]:
    if st.button("Save request"):
        ok = write_mode_request(req_mode)
        if ok:
            st.success(f"Saved requested mode: {req_mode}.")
        else:
            st.error("Failed to write mode request.")
with cols[2]:
    st.info(
        "Mode requests take effect on the next bot restart. "
        "Pilot/Live will still require arming (console prompt or env token) and will fail-closed to paper if not armed."
    )

st.subheader("Last decision")
st.json(STATE.get("last_decision", {}))

st.subheader("Recent risk governance")
st.json(recall_recent("risk_governance", limit=10))

st.subheader("Latest TradingView alert")
st.json(recall_recent("tv_alert", limit=1))

st.subheader("Execution KPIs (latest snapshot)")
st.dataframe(latest_exec_stats(limit=500))
