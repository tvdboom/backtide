"""Backtide.

Author: Mavs
Description: Entry point for the streamlit app.

"""

import streamlit as st

from backtide.utils.constants import DOCS_URL

st.set_page_config(
    page_title="Backtide",
    page_icon="images/icon transparent.png",
    layout="centered",
    initial_sidebar_state="expanded",
)

st.markdown(
    """
    <style>
        .stMainBlockContainer {
            max-width:46rem;
        }

        /* Fix sidebar width and disable resizing */
        section[data-testid="stSidebar"] {
            min-width: 12rem !important;
            max-width: 12rem !important;
        }

        /* Hide the resize handle and kill all pointer events on it */
        [data-testid="stSidebarResizeHandle"],
        [data-testid="stSidebarResizeHandle"] * {
            width: 0 !important;
            opacity: 0 !important;
            pointer-events: none !important;
            cursor: default !important;
        }

        /* Target the resize div Streamlit injects between sidebar and content */
        #root > div > div > div > div[style*="width: 6px"],
        #root > div > div > div > div[style*="cursor: col-resize"],
        div[style*="cursor: col-resize"] {
            cursor: default !important;
            pointer-events: none !important;
            background: transparent !important;
        }

        /* Hide the collapse button */
        [data-testid="stSidebarCollapseButton"] {
            display: none !important;
        }

        /* Image block: above the nav, reduced bottom margin */
        section[data-testid="stSidebar"] > div:first-child {
            display: flex;
            flex-direction: column;
        }

        /* Center the image container */
        [data-testid="stSidebar"] [data-testid="stVerticalBlock"] div:has(img) {
            display: flex;
            justify-content: center;
        }

        section[data-testid="stSidebar"] > div:first-child > div:has([data-testid="stImage"]) {
            order: -1;
            margin-bottom: -2.5rem !important;
            padding-bottom: 0 !important;
        }

        /* Hide fullscreen button on the logo image only */
        section[data-testid="stSidebar"] > div:first-child > div:has([data-testid="stImage"]) button,
        section[data-testid="stSidebar"] > div:first-child > div:has([data-testid="stImage"]) [data-testid="StyledFullScreenButton"] {
            display: none !important;
        }

        section[data-testid="stSidebar"] > div:first-child > div:has([data-testid="stSidebarNav"]) {
            margin-top: 0 !important;
            padding-top: 0 !important;
        }

        /* Footer: fixed to bottom, exactly 16rem wide matching sidebar */
        .sidebar-footer {
            position: fixed;
            bottom: 1rem;
            left: 0;
            width: 12rem;
            display: flex;
            justify-content: center;
            align-items: center;
            gap: 1.5rem;
            padding: 0.75rem 0;
            border-top: 1px solid rgba(255, 255, 255, 0.1);
            box-sizing: border-box;
            z-index: 999;
        }

        .sidebar-footer a {
            color: inherit;
            text-decoration: none;
            display: flex;
            align-items: center;
            gap: 0.35rem;
            opacity: 0.7;
            transition: opacity 0.2s;
        }

        .sidebar-footer a:hover {
            opacity: 1;
        }

        .sidebar-footer svg {
            width: 18px;
            height: 18px;
            fill: currentColor;
        }

        .sidebar-footer a:nth-child(2) svg {
            fill: none;
            stroke: currentColor;
            stroke-width: 2;
        }

        /* Docs button: fixed top-right corner */
        .docs-btn {
            position: fixed;
            top: 0.6rem;
            right: 0.8rem;
            z-index: 1000;
            display: flex;
            align-items: center;
            gap: 0.3rem;
            padding: 0.3rem 0.65rem;
            border-radius: 8px;
            border: 1px solid rgba(255,255,255,0.12);
            background: rgba(255,255,255,0.04);
            color: inherit;
            text-decoration: none;
            font-size: 13px;
            opacity: 0.55;
            transition: opacity 0.2s, background 0.2s;
        }
        .docs-btn:hover {
            opacity: 1;
            background: rgba(255,255,255,0.08);
        }
        .docs-btn svg {
            width: 15px;
            height: 15px;
            fill: none;
            stroke: currentColor;
            stroke-width: 2;
        }
    </style>
    """,
    unsafe_allow_html=True,
)


st.sidebar.markdown(
    f"""
    <div class="sidebar-footer">
        <a href="{DOCS_URL}" target="_blank" title="GitHub">
            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                <path d="M12 .297c-6.63 0-12 5.373-12 12 0 5.303 3.438 9.8 8.205 11.385.6.113.82-.258.82-.577
                0-.285-.01-1.04-.015-2.04-3.338.724-4.042-1.61-4.042-1.61C4.422 18.07 3.633 17.7
                3.633 17.7c-1.087-.744.084-.729.084-.729 1.205.084 1.838 1.236 1.838 1.236 1.07
                1.835 2.809 1.305 3.495.998.108-.776.417-1.305.76-1.605-2.665-.3-5.466-1.332-5.466-5.93
                0-1.31.465-2.38 1.235-3.22-.135-.303-.54-1.523.105-3.176 0 0 1.005-.322 3.3 1.23.96-.267
                1.98-.399 3-.405 1.02.006 2.04.138 3 .405 2.28-1.552 3.285-1.23 3.285-1.23.645 1.653.24
                2.873.12 3.176.765.84 1.23 1.91 1.23 3.22 0 4.61-2.805 5.625-5.475 5.92.42.36.81 1.096.81
                2.22 0 1.606-.015 2.896-.015 3.286 0 .315.21.69.825.57C20.565 22.092 24 17.592 24 12.297c0-6.627-5.373-12-12-12"/>
            </svg>
        </a>
        <a href="https://tvdboom.github.io/backtide" target="_blank" title="Documentation">
            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" style="fill:none;stroke:currentColor;">
                <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
                <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
            </svg>
        </a>
        <a href="https://pypi.org/project/backtide" target="_blank" title="PyPI">
            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                <path d="M11.826.11C9.886.11 8.673.965 8.673 2.29v2.047h3.282v.682H6.434C5.01
                5.02 3.957 5.942 3.568 7.65c-.45 1.975-.47 3.205 0 5.27.348 1.533 1.18 2.628
                2.604 2.628h1.685v-2.453c0-1.558 1.348-2.933 2.942-2.933h3.278c1.31 0 2.372-1.08
                2.372-2.396V2.29C16.449.965 15.2.11 11.826.11zm-1.782 1.39c.49 0 .89.405.89.904a.898.898
                0 0 1-.89.905.898.898 0 0 1-.89-.905c0-.499.4-.904.89-.904zm.804 21.39c1.94 0 3.153-.855
                3.153-2.18v-2.048H10.72v-.682h5.52c1.426 0 2.478-.921 2.868-2.628.45-1.975.47-3.205
                0-5.27-.348-1.534-1.18-2.628-2.604-2.628h-1.685v2.453c0 1.558-1.348 2.933-2.942
                2.933H8.598c-1.31 0-2.372 1.08-2.372 2.396v4.474C6.226 22.635 7.474 22.89 10.848
                22.89zm1.782-1.39a.898.898 0 0 1-.89-.905c0-.499.4-.904.89-.904.49 0 .89.405.89.904a.898.898
                0 0 1-.89.905z"/>
            </svg>
        </a>
        <a href="mailto:m.524687@gmail.com" title="Email">
            <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg">
                <path d="M20 4H4a2 2 0 0 0-2 2v12a2 2 0 0 0 2 2h16a2 2 0 0 0 2-2V6a2 2 0 0
                0-2-2zm0 2-8 5-8-5h16zm0 12H4V8.236l8 5 8-5V18z"/>
            </svg>
        </a>
    </div>
    """,
    unsafe_allow_html=True,
)

st.sidebar.image("images/logo transparent.png", width=120)

# Define pages
experiment = st.Page(
    "experiment.py",
    title="Experiment",
    icon=":material/science:",
)
results = st.Page(
    "results.py",
    title="Results",
    icon=":material/fact_check:",
)
download = st.Page(
    "download.py",
    title="Download",
    icon=":material/cloud_download:",
)
storage = st.Page(
    "storage.py",
    title="Storage",
    icon=":material/database:",
)
analysis = st.Page(
    "analysis.py",
    title="Analysis",
    icon=":material/assessment:",
)
indicators = st.Page(
    "indicators.py",
    title="Indicators",
    icon=":material/show_chart:",
)


PAGES_URLS = {
    "Experiment": f"{DOCS_URL}/user_guide/backtest/experiment/",
    "Results": f"{DOCS_URL}/user_guide/backtest/results/",
    "Indicators": f"{DOCS_URL}/user_guide/backtest/indicators/",
    "Download": f"{DOCS_URL}/user_guide/data/",
    "Storage": f"{DOCS_URL}/user_guide/storage/",
    "Analysis": f"{DOCS_URL}/user_guide/data/analysis",
}

pg = st.navigation(
    {"Backtest": [experiment, results, indicators], "Data": [download, storage, analysis]}
)

# Inject the docs button for the current page at the top of the content area
st.html(
    f"""
    <a class="docs-btn" href="{PAGES_URLS[pg.title]}" target="_blank" title="Documentation">
        <svg viewBox="0 0 24 24" xmlns="http://www.w3.org/2000/svg" stroke-linecap="round" stroke-linejoin="round">
            <path d="M4 19.5A2.5 2.5 0 0 1 6.5 17H20"/>
            <path d="M6.5 2H20v20H6.5A2.5 2.5 0 0 1 4 19.5v-15A2.5 2.5 0 0 1 6.5 2z"/>
        </svg>
        Docs
    </a>
    """,
)

pg.run()
