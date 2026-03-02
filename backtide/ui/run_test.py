
import streamlit as st

st.title("New Backtest")
st.divider()

with st.form("backtest_form"):
    col1, col2 = st.columns(2)
    with col1:
        strategy = st.text_input("Strategy Name", placeholder="e.g. Mean Reversion v2")
        symbol = st.text_input("Symbol / Asset", placeholder="e.g. BTCUSD")
        start = st.date_input("Start Date")
    with col2:
        timeframe = st.selectbox("Timeframe", ["1m", "5m", "15m", "1h", "4h", "1D", "1W"])
        capital = st.number_input("Initial Capital ($)", value=10_000, step=1_000)
        end = st.date_input("End Date")

    commission = st.slider("Commission (%)", 0.0, 1.0, 0.05, step=0.01)
    submitted = st.form_submit_button("🚀  Run Backtest", use_container_width=True)

if submitted:
    st.success(f"Backtest **{strategy}** on **{symbol}** queued successfully!")
