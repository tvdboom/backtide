"""Streamlit Backtesting Application.

This application provides a simple UI for managing trading or simulation
backtests.

"""

import streamlit as st

st.set_page_config(
    page_title="Backtide",
    page_icon="images/icon transparent.png",
    layout="wide",
    initial_sidebar_state="expanded",
)

# Define pages
new_backtest = st.Page(
    "ui/new_backtest.py",
    title="New Backtest",
    icon=":material/science:",
)
results = st.Page(
    "ui/results.py",
    title="Results",
    icon=":material/analytics:",
)

# Increase font-size of sidebar
st.markdown("""
<style>
[data-testid="stSidebarNav"] li span,
[data-testid="stSidebarNavItems"] span,
nav[data-testid="stSidebarNav"] ul li:first-child span,
section[data-testid="stSidebar"] nav span {
    font-size: 1.2rem !important;
}
</style>
""", unsafe_allow_html=True)

st.markdown(
        f"""
        <style>
            [data-testid="stSidebarNav"] {{
                background-image: url(http://placekitten.com/200/200);
                background-repeat: no-repeat;
                padding-top: {200 - 40}px;
                background-position: 20px 20px;
            }}
        </style>
        """,
        unsafe_allow_html=True,
    )

# st.logo("images/logo transparent.png", size="large", icon_image="images/icon transparent.png")

st.sidebar.image("images/logo transparent.png")

pg = st.navigation({"Backtest": [new_backtest, results]})
pg.run()
