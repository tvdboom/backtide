"""Backtide.

Author: Mavs
Description: Backtest results page.

"""

import streamlit as st


st.title("Results")
st.divider()

with st.expander("📈 Mean Reversion v2 · BTCUSD · 1H — 28 Feb 2026", expanded=True):
    c1, c2, c3, c4, c5 = st.columns(5)
    c1.metric("Total Return", "+42.8%", delta="+42.8%")
    c2.metric("Sharpe Ratio", "1.74")
    c3.metric("Max Drawdown", "-12.3%", delta="-12.3%", delta_color="inverse")
    c4.metric("Win Rate", "61%")
    c5.metric("Total Trades", "348")

with st.expander("📉 Momentum Scalper · ETHUSD · 15m — 25 Feb 2026"):
    c1, c2, c3, c4, c5 = st.columns(5)
    c1.metric("Total Return", "-5.1%", delta="-5.1%")
    c2.metric("Sharpe Ratio", "0.38")
    c3.metric("Max Drawdown", "-18.7%", delta="-18.7%", delta_color="inverse")
    c4.metric("Win Rate", "47%")
    c5.metric("Total Trades", "1,204")
