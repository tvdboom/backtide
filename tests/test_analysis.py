"""Backtide.

Author: Mavs
Description: Unit tests for the analysis module (plots and compute_statistics).

"""

from typing import cast
from unittest.mock import MagicMock, patch

import numpy as np
import pandas as pd
import plotly.graph_objects as go
import pytest

from backtide.analysis import (
    compute_statistics,
    plot_candlestick,
    plot_cash_holdings,
    plot_correlation,
    plot_dividends,
    plot_drawdown,
    plot_pnl,
    plot_price,
    plot_returns,
    plot_seasonality,
    plot_volatility,
    plot_volume,
    plot_vwap,
)
from backtide.analysis.utils import (
    _check_columns,
    _get_currency_symbol,
    _plot,
    _resolve_dt,
)
from backtide.backtest import RunResult
from backtide.core.data import Currency

# ─────────────────────────────────────────────────────────────────────────────
# Fixtures
# ─────────────────────────────────────────────────────────────────────────────


@pytest.fixture
def daily_bars():
    """Return a single-symbol daily OHLCV DataFrame with 60 trading days."""
    n = 60
    dates = pd.bdate_range("2024-01-02", periods=n, tz="UTC")
    rng = np.random.default_rng(42)
    close = 100 + np.cumsum(rng.normal(0, 1, n))
    return pd.DataFrame(
        {
            "symbol": "AAPL",
            "dt": dates,
            "open_ts": dates.astype("int64") // 10**9,
            "open": close - rng.uniform(0, 1, n),
            "high": close + rng.uniform(0, 2, n),
            "low": close - rng.uniform(0, 2, n),
            "close": close,
            "adj_close": close * 0.99,
            "volume": rng.integers(1_000, 100_000, n),
            "currency": "USD",
        }
    )


@pytest.fixture
def multi_bars(daily_bars):
    """Return a two-symbol daily OHLCV DataFrame."""
    msft = daily_bars.copy()
    msft["symbol"] = "MSFT"
    msft["close"] = daily_bars["close"] * 1.5 + np.random.default_rng(7).normal(0, 1, len(msft))
    msft["adj_close"] = msft["close"] * 0.99
    return pd.concat([daily_bars, msft], ignore_index=True)


@pytest.fixture
def intraday_bars():
    """Return a single-symbol intraday DataFrame (hourly, 3 days)."""
    dates = pd.date_range("2024-06-03 09:00", periods=24, freq="h", tz="UTC")
    dates = dates.append(pd.date_range("2024-06-04 09:00", periods=24, freq="h", tz="UTC"))
    dates = dates.append(pd.date_range("2024-06-05 09:00", periods=24, freq="h", tz="UTC"))
    n = len(dates)
    rng = np.random.default_rng(99)
    close = 150 + np.cumsum(rng.normal(0, 0.5, n))
    return pd.DataFrame(
        {
            "symbol": "AAPL",
            "dt": dates,
            "open": close - 0.1,
            "high": close + 0.5,
            "low": close - 0.5,
            "close": close,
            "adj_close": close,
            "volume": rng.integers(100, 5000, n),
            "currency": "USD",
        }
    )


@pytest.fixture
def dividend_data():
    """Return a small dividends DataFrame."""
    return pd.DataFrame(
        {
            "symbol": ["AAPL", "AAPL", "MSFT"],
            "dt": pd.to_datetime(["2024-02-10", "2024-05-10", "2024-03-15"], utc=True),
            "amount": [0.24, 0.25, 0.75],
            "currency": "USD",
        }
    )


# ─────────────────────────────────────────────────────────────────────────────
# Utility tests
# ─────────────────────────────────────────────────────────────────────────────


class TestResolveDt:
    """Tests for the _resolve_dt helper."""

    def test_passthrough_with_dt(self, daily_bars):
        """Return data unchanged when 'dt' column already exists."""
        result = _resolve_dt(daily_bars)
        assert "dt" in result.columns
        assert result is daily_bars

    def test_renames_datetime_column(self, daily_bars):
        """Copy 'datetime' column to 'dt' when only 'datetime' exists."""
        df = daily_bars.rename(columns={"dt": "datetime"})
        result = _resolve_dt(df)
        assert "dt" in result.columns
        assert result is not df

    def test_converts_open_ts(self, daily_bars):
        """Convert 'open_ts' unix-seconds column to 'dt'."""
        df = daily_bars.drop(columns=["dt"])
        df["open_ts"] = daily_bars["dt"].astype("int64") // 10**9
        result = _resolve_dt(df)
        assert "dt" in result.columns
        assert result is not df

    def test_no_datetime_column(self):
        """Return data as-is when no datetime-related column exists."""
        df = pd.DataFrame({"symbol": ["X"], "close": [100]})
        result = _resolve_dt(df)
        assert "dt" not in result.columns

    def test_converts_ts_column(self, daily_bars):
        """Convert 'ts' unix-seconds column to 'dt'."""
        df = daily_bars.drop(columns=["dt", "open_ts"])
        df["ts"] = daily_bars["dt"].astype("int64") // 10**9
        result = _resolve_dt(df)
        assert "dt" in result.columns
        assert result is not df

    def test_converts_ex_date_column(self, daily_bars):
        """Convert 'ex_date' unix-seconds column to 'dt'."""
        df = daily_bars.drop(columns=["dt", "open_ts"])
        df["ex_date"] = daily_bars["dt"].astype("int64") // 10**9
        result = _resolve_dt(df)
        assert "dt" in result.columns
        assert result is not df


class TestCheckColumns:
    """Tests for the _check_columns helper."""

    def test_passes_with_valid_columns(self, daily_bars):
        """No error when all required columns are present."""
        _check_columns(daily_bars, ["symbol", "dt", "close"], "test")

    def test_raises_on_missing_columns(self, daily_bars):
        """Raise ValueError listing the missing columns."""
        with pytest.raises(ValueError, match="missing_col"):
            _check_columns(daily_bars, ["symbol", "missing_col"], "test")


class TestGetCurrencySymbol:
    """Tests for the _get_currency_symbol helper."""

    def test_single_currency(self, daily_bars):
        """Return Currency when all rows share the same currency."""
        result = _get_currency_symbol(daily_bars)
        assert result is not None
        assert result.symbol == "$"

    def test_mixed_currencies(self, daily_bars):
        """Return None when rows have different currencies."""
        df = daily_bars.copy()
        df.loc[0, "currency"] = "EUR"
        assert _get_currency_symbol(df) is None

    def test_no_currency_column(self):
        """Return None when 'currency' column is absent."""
        df = pd.DataFrame({"symbol": ["X"], "close": [1.0]})
        assert _get_currency_symbol(df) is None

    def test_all_nan_currency(self):
        """Return None when all currency values are NaN."""
        df = pd.DataFrame({"currency": [None, None, None]})
        assert _get_currency_symbol(df) is None


class TestPlotHelper:
    """Tests for the _plot layout helper."""

    def test_display_none_returns_figure(self):
        """Return a go.Figure when display=None."""
        fig = go.Figure()
        result = _plot(fig, display=None)
        assert isinstance(result, go.Figure)

    def test_display_false_returns_none(self):
        """Return None when display=False."""
        result = _plot(go.Figure(), display=False)
        assert result is None

    def test_string_title(self):
        """Apply a string title to the figure layout."""
        fig = _plot(go.Figure(), title="Test", display=None)
        assert fig.layout.title.text == "Test"

    def test_dict_title(self):
        """Apply a dict title configuration."""
        fig = _plot(go.Figure(), title={"text": "Custom"}, display=None)
        assert fig.layout.title.text == "Custom"

    def test_string_legend(self):
        """Position legend using a string shorthand."""
        fig = _plot(go.Figure(), legend="lower left", display=None)
        assert fig.layout.showlegend is True

    def test_dict_legend(self):
        """Configure legend using a dict."""
        fig = _plot(go.Figure(), legend={"x": 0.5}, display=None)
        assert fig.layout.showlegend is True

    def test_no_legend(self):
        """Hide legend when legend=None."""
        fig = _plot(go.Figure(), legend=None, display=None)
        assert fig.layout.showlegend is False

    def test_axis_limits(self):
        """Apply x and y axis limits."""
        fig = _plot(go.Figure(), xlim=(0, 10), ylim=(0, 100), display=None)
        assert fig.layout.xaxis.range == (0, 10)
        assert fig.layout.yaxis.range == (0, 100)

    def test_save_html(self, tmp_path):
        """Save figure as HTML file."""
        path = tmp_path / "test_plot.html"
        _plot(go.Figure(), filename=str(path), display=False)
        assert path.exists()

    def test_save_no_extension(self, tmp_path):
        """Default to .html when filename has no extension."""
        path = tmp_path / "test_plot"
        _plot(go.Figure(), filename=str(path), display=False)
        assert (tmp_path / "test_plot.html").exists()

    @patch.object(go.Figure, "show")
    def test_display_true_calls_show(self, mock_show):
        """Call fig.show() when display=True."""
        result = _plot(go.Figure(), display=True)
        mock_show.assert_called_once()
        assert result is None

    @patch.object(go.Figure, "write_image")
    def test_save_image(self, mock_write, tmp_path):
        """Save figure as an image file when extension is not .html."""
        path = tmp_path / "test_plot.png"
        _plot(go.Figure(), filename=str(path), display=False)
        mock_write.assert_called_once()

    def test_figsize_custom(self):
        """Apply custom figsize to the layout."""
        fig = _plot(go.Figure(), figsize=(1200, 800), display=None)
        assert fig.layout.width == 1200
        assert fig.layout.height == 800

    def test_axis_labels(self):
        """Apply x and y axis labels to the layout."""
        fig = _plot(go.Figure(), xlabel="X", ylabel="Y", display=None)
        assert fig.layout.xaxis.title.text == "X"
        assert fig.layout.yaxis.title.text == "Y"

    def test_no_title(self):
        """No title when title=None."""
        fig = _plot(go.Figure(), title=None, display=None)
        assert fig.layout.title.text is None


# ─────────────────────────────────────────────────────────────────────────────
# Plot tests
# ─────────────────────────────────────────────────────────────────────────────


class TestPlotPrice:
    """Tests for plot_price."""

    def test_single_symbol(self, daily_bars):
        """Return a figure with one trace for a single symbol."""
        fig = plot_price(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1

    def test_multi_symbol(self, multi_bars):
        """Return one trace per symbol."""
        fig = plot_price(multi_bars, display=None)
        assert len(fig.data) == 2

    def test_custom_price_col(self, daily_bars):
        """Accept a custom price column."""
        fig = plot_price(daily_bars, price_col="close", display=None)
        assert isinstance(fig, go.Figure)

    def test_missing_column(self, daily_bars):
        """Raise ValueError when a required column is missing."""
        with pytest.raises(ValueError, match="requires column"):
            plot_price(daily_bars.drop(columns=["symbol"]), display=None)

    def test_currency_in_ylabel(self, daily_bars):
        """Include currency symbol in the y-axis label."""
        fig = plot_price(daily_bars, display=None)
        assert "$" in fig.layout.yaxis.title.text

    def test_with_line_indicator(self, daily_bars):
        """Overlay a single-column indicator adds extra traces."""
        ind = MagicMock()
        ind.compute.return_value = pd.DataFrame({"sma": daily_bars["close"].rolling(5).mean()})
        fig = plot_price(daily_bars, indicators={"SMA": ind}, display=None)
        # 1 price trace + 1 indicator trace
        assert len(fig.data) == 2

    def test_with_band_indicator(self, daily_bars):
        """Overlay a two-column indicator adds band traces."""
        ind = MagicMock()
        ind.compute.return_value = pd.DataFrame(
            {
                "upper": daily_bars["close"] + 2,
                "lower": daily_bars["close"] - 2,
            }
        )
        fig = plot_price(daily_bars, indicators={"BB": ind}, display=None)
        # 1 price trace + 2 band traces
        assert len(fig.data) == 3

    def test_with_list_indicator(self, daily_bars):
        """Overlay indicators passed as a list adds extra traces."""
        ind = MagicMock()
        ind.__class__.__name__ = "TestIndicator"
        ind.compute.return_value = pd.DataFrame({"sma": daily_bars["close"].rolling(5).mean()})
        fig = plot_price(daily_bars, indicators=[ind], display=None)
        # 1 price trace + 1 indicator trace
        assert len(fig.data) == 2


class TestPlotCandlestick:
    """Tests for plot_candlestick."""

    def test_returns_figure(self, daily_bars):
        """Return a figure with a Candlestick trace."""
        fig = plot_candlestick(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert any(isinstance(t, go.Candlestick) for t in fig.data)

    def test_missing_column(self, daily_bars):
        """Raise ValueError when OHLC columns are missing."""
        with pytest.raises(ValueError, match="requires column"):
            plot_candlestick(daily_bars.drop(columns=["open"]), display=None)


class TestPlotCorrelation:
    """Tests for plot_correlation."""

    def test_two_symbols(self, multi_bars):
        """Return a figure with a Heatmap trace for two symbols."""
        fig = plot_correlation(multi_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert any(isinstance(t, go.Heatmap) for t in fig.data)

    def test_values_in_range(self, multi_bars):
        """Correlation values should be in [-1, 1]."""
        fig = plot_correlation(multi_bars, display=None)
        heatmap = next(t for t in fig.data if isinstance(t, go.Heatmap))
        z = np.array(heatmap.z)
        assert np.all((z >= -1) & (z <= 1))


class TestPlotDividends:
    """Tests for plot_dividends."""

    def test_returns_figure(self, dividend_data):
        """Return a figure for dividend data."""
        fig = plot_dividends(dividend_data, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) > 0

    def test_missing_column(self, dividend_data):
        """Raise ValueError when 'amount' column is missing."""
        with pytest.raises(ValueError, match="requires column"):
            plot_dividends(dividend_data.drop(columns=["amount"]), display=None)

    def test_multi_symbol_dividends(self, dividend_data):
        """Return traces for each symbol in dividend data."""
        fig = plot_dividends(dividend_data, display=None)
        assert len(fig.data) > 1


class TestPlotDrawdown:
    """Tests for plot_drawdown."""

    def test_single_symbol(self, daily_bars):
        """Return a figure with one filled trace."""
        fig = plot_drawdown(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1

    def test_multi_symbol(self, multi_bars):
        """Return one trace per symbol."""
        fig = plot_drawdown(multi_bars, display=None)
        assert len(fig.data) == 2

    def test_values_non_positive(self, daily_bars):
        """Drawdown values should be <= 0."""
        fig = plot_drawdown(daily_bars, display=None)
        y = np.array(fig.data[0].y, dtype=float)
        assert np.all(y[~np.isnan(y)] <= 0)


class TestPlotReturns:
    """Tests for plot_returns."""

    def test_returns_figure(self, daily_bars):
        """Return a figure with a Histogram trace."""
        fig = plot_returns(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert any(isinstance(t, go.Histogram) for t in fig.data)

    def test_multi_symbol(self, multi_bars):
        """Return one histogram (plus a normal-fit overlay) per symbol."""
        fig = plot_returns(multi_bars, display=None)
        histograms = [t for t in fig.data if isinstance(t, go.Histogram)]
        assert len(histograms) == 2


class TestPlotSeasonality:
    """Tests for plot_seasonality."""

    def test_daily_year_month(self, daily_bars):
        """Daily data produces a year x month heatmap."""
        fig = plot_seasonality(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        heatmap = next(t for t in fig.data if isinstance(t, go.Heatmap))
        # x-axis labels should be month abbreviations
        assert any("Jan" in str(x) or "Feb" in str(x) for x in heatmap.x)

    def test_intraday_dow_hour(self, intraday_bars):
        """Intraday data produces a day-of-week x hour heatmap."""
        fig = plot_seasonality(intraday_bars, display=None)
        heatmap = next(t for t in fig.data if isinstance(t, go.Heatmap))
        # x-axis labels should be hour format
        assert any(":00" in str(x) for x in heatmap.x)

    def test_multi_symbol_raises(self, multi_bars):
        """Raise ValueError when data contains multiple symbols."""
        with pytest.raises(ValueError, match="single symbol"):
            plot_seasonality(multi_bars, display=None)


class TestPlotVolatility:
    """Tests for plot_volatility."""

    def test_returns_figure(self, daily_bars):
        """Return a figure with one Scatter trace."""
        fig = plot_volatility(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1

    def test_custom_window(self, daily_bars):
        """Accept a custom rolling window."""
        fig = plot_volatility(daily_bars, window=5, display=None)
        assert isinstance(fig, go.Figure)


class TestPlotVolume:
    """Tests for plot_volume."""

    def test_returns_figure(self, daily_bars):
        """Return a figure with traces for volume data."""
        fig = plot_volume(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) >= 1

    def test_multi_symbol(self, multi_bars):
        """Return one Bar trace per symbol."""
        fig = plot_volume(multi_bars, display=None)
        assert len(fig.data) == 2


class TestPlotVwap:
    """Tests for plot_vwap."""

    def test_daily_traces(self, daily_bars):
        """Return two traces per symbol (Close + VWAP) for daily data."""
        fig = plot_vwap(daily_bars, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 2

    def test_intraday_resets(self, intraday_bars):
        """Intraday VWAP resets daily (each day starts at typical price)."""
        fig = plot_vwap(intraday_bars, display=None)
        assert len(fig.data) == 2

    def test_multi_symbol_vwap(self, multi_bars):
        """Return two traces per symbol for multi-symbol data."""
        fig = plot_vwap(multi_bars, display=None)
        # 2 symbols x 2 traces (Close + VWAP) = 4
        assert len(fig.data) == 4

    def test_missing_column(self, daily_bars):
        """Raise ValueError when volume column is missing."""
        with pytest.raises(ValueError, match="requires column"):
            plot_vwap(daily_bars.drop(columns=["volume"]), display=None)


# ─────────────────────────────────────────────────────────────────────────────
# compute_statistics tests
# ─────────────────────────────────────────────────────────────────────────────


class TestComputeStatistics:
    """Tests for compute_statistics."""

    def test_single_symbol(self, daily_bars):
        """Return a one-row DataFrame for a single symbol."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars))
        assert len(result) == 1
        assert result.iloc[0]["symbol"] == "AAPL"

    def test_multi_symbol(self, multi_bars):
        """Return one row per symbol."""
        result = cast(pd.DataFrame, compute_statistics(multi_bars))
        assert len(result) == 2
        assert set(result["symbol"]) == {"AAPL", "MSFT"}

    def test_expected_columns(self, daily_bars):
        """Result contains the expected metric columns."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars))
        for col in ("cagr", "ann_volatility", "sharpe", "max_dd", "win_rate"):
            assert col in result.columns, f"Missing column: {col}"

    def test_custom_price_col(self, daily_bars):
        """Accept a custom price column."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars, price_col="close"))
        assert len(result) == 1

    def test_max_drawdown_non_positive(self, daily_bars):
        """Max drawdown should be <= 0."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars))
        assert result.iloc[0]["max_dd"] <= 0

    def test_win_rate_in_range(self, daily_bars):
        """Win rate should be between 0 and 1 (fraction)."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars))
        wr = result.iloc[0]["win_rate"]
        assert 0 <= wr <= 1


class _StubSample:
    """Minimal duck-typed stand-in for `EquitySample`."""

    def __init__(self, ts: int, equity: float, cash: dict[str, float] | None = None):
        self.timestamp = ts
        self.equity = equity
        self.cash = cash or {}


class _StubRun:
    """Minimal duck-typed stand-in for `RunResult`."""

    def __init__(
        self,
        name: str,
        equity: list[float],
        start: int = 1_700_000_000,
        trades: list | None = None,
        orders: list | None = None,
        base_currency: str = "USD",
        *,
        is_benchmark: bool = False,
    ):
        self.strategy_name = name
        self.is_benchmark = is_benchmark
        # 1-day spacing (in seconds) so the x axis renders sensibly.
        self.equity_curve = [
            _StubSample(start + i * 86_400, e, cash={base_currency: e})
            for i, e in enumerate(equity)
        ]
        self.trades = trades or []
        self.orders = orders or []
        self.base_currency = Currency(base_currency)


def _run_result(
    name: str,
    equity: list[float],
    start: int = 1_700_000_000,
    trades: list | None = None,
    orders: list | None = None,
    base_currency: str = "USD",
    *,
    is_benchmark: bool = False,
) -> RunResult:
    """Build a `_StubRun` and expose it as a `RunResult` for type checkers."""
    return cast(
        RunResult,
        _StubRun(name, equity, start, trades, orders, base_currency, is_benchmark=is_benchmark),
    )


class TestPlotPnl:
    """Tests for plot_pnl."""

    def test_returns_figure(self):
        """Plotting one strategy returns a figure with one trace (no drawdown)."""
        run = _run_result("S1", [10_000.0, 10_500.0, 11_000.0])
        fig = plot_pnl([run], drawdown=False, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1

    def test_drawdown_default_adds_subplot(self):
        """`drawdown=True` (default) yields two traces per run on two rows."""
        run = _run_result("S1", [10_000.0, 10_500.0, 11_000.0])
        fig = plot_pnl([run], display=None)
        assert len(fig.data) == 2  # PnL + drawdown
        # Subplots: yaxis2 must exist for the drawdown panel.
        assert fig.layout.yaxis2 is not None

    def test_drawdown_absolute_when_not_normalized(self):
        """Default mode plots drawdown as absolute currency, not percent."""
        run = _run_result("S1", [100.0, 120.0, 90.0])
        fig = plot_pnl([run], drawdown=True, normalize=False, display=None)
        dd = np.array(fig.data[1].y, dtype=float)
        assert dd.tolist() == [0.0, 0.0, -30.0]

    def test_drawdown_percent_when_normalized(self):
        """Normalized mode plots drawdown as a percentage."""
        run = _run_result("S1", [100.0, 120.0, 90.0])
        fig = plot_pnl([run], drawdown=True, normalize=True, display=None)
        dd = np.array(fig.data[1].y, dtype=float)
        assert dd[0] == 0.0
        assert dd[1] == 0.0
        assert dd[2] == pytest.approx(-25.0)

    def test_one_trace_per_strategy(self):
        """Plotting N strategies yields N traces (no drawdown)."""
        runs = [
            _run_result("S1", [10_000.0, 10_500.0, 11_000.0]),
            _run_result("S2", [10_000.0, 9_500.0, 9_800.0]),
            _run_result("Benchmark (SPY)", [10_000.0, 10_100.0, 10_200.0]),
        ]
        fig = plot_pnl(runs, drawdown=False, display=None)
        assert len(fig.data) == 3
        assert {t.name for t in fig.data} == {"S1", "S2", "Benchmark (SPY)"}

    def test_absolute_pnl_starts_at_zero(self):
        """Absolute PnL should start at zero for every strategy."""
        run = _run_result("S1", [10_000.0, 10_500.0, 11_000.0])
        fig = plot_pnl([run], drawdown=False, display=None)
        y = np.array(fig.data[0].y, dtype=float)
        assert y[0] == 0.0
        assert y[-1] == 1_000.0

    def test_relative_pnl_in_percent(self):
        """Relative mode plots returns as a percentage of the start equity."""
        run = _run_result("S1", [200.0, 220.0, 250.0])
        fig = plot_pnl([run], normalize=True, drawdown=False, display=None)
        y = np.array(fig.data[0].y, dtype=float)
        assert y[0] == 0.0
        assert y[-1] == pytest.approx(25.0)  # 250 / 200 - 1 = 25 %

    def test_skips_runs_without_equity(self):
        """Runs without an equity curve are silently skipped."""

        class _Empty:
            strategy_name = "empty"
            equity_curve = None
            base_currency = Currency.USD

        run = _run_result("S1", [10_000.0, 10_100.0])
        fig = plot_pnl([cast(RunResult, _Empty()), run], drawdown=False, display=None)
        assert len(fig.data) == 1
        assert fig.data[0].name == "S1"

    def test_benchmark_drawn_dashed(self):
        """The auto-injected benchmark run is rendered with a dashed line."""
        runs = [
            _run_result("S1", [10_000.0, 10_500.0]),
            _run_result("Benchmark", [10_000.0, 10_100.0], is_benchmark=True),
        ]
        fig = plot_pnl(runs, drawdown=False, display=None)
        bench = next(t for t in fig.data if t.name == "Benchmark")
        # The reserved exact benchmark name is rendered with benchmark styling.
        assert bench.line.dash == "dash"

    def test_empty_input_raises(self):
        """An empty `runs` list should raise a ValueError."""
        with pytest.raises(ValueError, match="cannot be empty"):
            plot_pnl([], drawdown=False, display=None)


# ─────────────────────────────────────────────────────────────────────────────
# Stubs for trade-/order-aware plot tests
# ─────────────────────────────────────────────────────────────────────────────


class _StubTrade:
    """Minimal duck-typed stand-in for `Trade`."""

    def __init__(
        self,
        symbol: str,
        entry_ts: int,
        exit_ts: int,
        entry_price: float,
        exit_price: float,
        quantity: int,
        pnl: float,
    ):
        self.symbol = symbol
        self.entry_ts = entry_ts
        self.exit_ts = exit_ts
        self.entry_price = entry_price
        self.exit_price = exit_price
        self.quantity = quantity
        self.pnl = pnl


class _StubOrder:
    """Minimal duck-typed stand-in for `Order`."""

    def __init__(self, symbol: str, quantity: int):
        self.symbol = symbol
        self.quantity = quantity


class _StubOrderRecord:
    """Minimal duck-typed stand-in for `OrderRecord`."""

    def __init__(
        self,
        symbol: str,
        quantity: int,
        ts: int,
        status: str = "filled",
        *,
        fill_price: float = 100.0,
        commission: float = 0.0,
    ):
        self.order = _StubOrder(symbol, quantity)
        self.timestamp = ts
        self.status = status
        self.fill_price = fill_price
        self.commission = commission


class _StubInstrument:
    """Minimal duck-typed stand-in for `Instrument` with quote currency."""

    def __init__(self, symbol: str, quote: str):
        self.symbol = symbol
        self.quote = quote


def _make_run_with_trades(
    name: str = "S1",
    pnls: tuple[float, ...] = (50.0, -30.0, 20.0, -10.0),
) -> RunResult:
    """Build a `_StubRun` with synthetic trades and matching orders."""
    base = 1_700_000_000
    trades = [
        _StubTrade(
            symbol="AAPL",
            entry_ts=base + i * 86_400,
            exit_ts=base + (i + 1) * 86_400,
            entry_price=100.0,
            exit_price=100.0 + p / 10,
            quantity=10 if i % 2 == 0 else -10,
            pnl=p,
        )
        for i, p in enumerate(pnls)
    ]
    orders = []
    for i, t in enumerate(trades):
        orders.append(_StubOrderRecord("AAPL", t.quantity, t.entry_ts))
        orders.append(_StubOrderRecord("AAPL", -t.quantity, t.exit_ts))
        if i == 0:
            orders.append(_StubOrderRecord("MSFT", 5, t.entry_ts + 1_000))
    return _run_result(
        name,
        [10_000.0 + sum(pnls[: i + 1]) for i in range(len(pnls))],
        trades=trades,
        orders=orders,
    )


# ─────────────────────────────────────────────────────────────────────────────
# Tests — new strategy-result plots
# ─────────────────────────────────────────────────────────────────────────────


class TestPlotRollingReturns:
    """Tests for plot_rolling_returns."""

    def test_returns_figure(self):
        """Builds one rolling-return trace per run."""
        from backtide.analysis import plot_rolling_returns

        run = _run_result("S1", [100.0 + i for i in range(50)])
        fig = plot_rolling_returns([run], window=10, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1

    def test_skips_short_runs(self):
        """Runs with fewer samples than `window` are silently skipped."""
        from backtide.analysis import plot_rolling_returns

        short = _run_result("S1", [100.0, 101.0, 102.0])
        fig = plot_rolling_returns([short], window=30, display=None)
        # No traces, but a figure with the placeholder annotation.
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 0


class TestPlotCashHoldings:
    """Tests for plot_cash_holdings."""

    def test_single_currency_labels_strategy_and_axis(self):
        """Single-currency strategies use strategy names and currency in y label."""
        base = 1_700_000_000
        run_a = _run_result(
            "S1",
            [10_000.0, 9_000.0],
            orders=[_StubOrderRecord("AAPL", 10, base, fill_price=100.0)],
            base_currency="USD",
        )
        run_b = _run_result(
            "S2",
            [10_000.0, 9_500.0],
            orders=[_StubOrderRecord("AAPL", 5, base + 60, fill_price=100.0)],
            base_currency="USD",
        )

        fig = plot_cash_holdings([run_a, run_b], display=None)
        assert isinstance(fig, go.Figure)
        assert {t.name for t in fig.data} == {"S1", "S2"}
        assert fig.layout.yaxis.title.text == "Cash ($)"

    def test_multi_currency_groups_by_strategy(self):
        """Multi-currency strategies show currency labels under a strategy legend group."""
        run = _run_result(
            "Global",
            [10_000.0, 10_100.0],
            base_currency="USD",
        )
        run.equity_curve[0].cash = {"USD": 10_000.0, "EUR": 0.0}
        run.equity_curve[1].cash = {"USD": 9_000.0, "EUR": 1_000.0}

        fig = plot_cash_holdings([run], display=None)
        assert isinstance(fig, go.Figure)
        assert {t.name for t in fig.data} == {"USD", "EUR"}
        assert all(t.legendgroup == "Global" for t in fig.data)
        assert fig.data[0].legendgrouptitle.text == "Global"
        assert fig.layout.yaxis.title.text == "Cash"

    def test_sparse_currency_buckets_keep_timestamps_aligned(self):
        """Currencies that only appear in some snapshots stay aligned on the time axis.

        Regression: the previous implementation built per-currency series via a
        ``defaultdict(list)``-style ``y`` array, then reused the *full* equity
        curve as the x-axis. If a bucket disappeared mid-run (e.g. ``USD`` is
        removed when the bucket is fully debited), the per-currency line was
        anchored to the first N timestamps instead of the timestamps where the
        currency actually existed.
        """
        run = _run_result(
            "Sparse",
            [10_000.0, 10_000.0, 10_000.0, 10_000.0],
            base_currency="EUR",
        )
        # USD only exists in snapshots 1 and 3 — the other bars don't include it.
        run.equity_curve[0].cash = {"EUR": 10_000.0}
        run.equity_curve[1].cash = {"EUR": 5_000.0, "USD": 100.0}
        run.equity_curve[2].cash = {"EUR": 5_000.0}
        run.equity_curve[3].cash = {"EUR": 4_000.0, "USD": 200.0}

        fig = plot_cash_holdings([run], display=None)
        traces = {t.name: t for t in fig.data}
        # EUR appears in every snapshot, USD only in two.
        assert len(traces["EUR"].x) == 4
        assert len(traces["EUR"].y) == 4
        assert len(traces["USD"].x) == 2
        assert len(traces["USD"].y) == 2
        # The two USD points line up with snapshots 1 and 3 (not 0 and 1).
        assert traces["USD"].y == (100.0, 200.0)
        assert traces["USD"].x[0] == traces["EUR"].x[1]
        assert traces["USD"].x[1] == traces["EUR"].x[3]


class TestPlotRollingSharpe:
    """Tests for plot_rolling_sharpe."""

    def test_returns_figure(self):
        """Computes a rolling-Sharpe trace from a noisy equity curve."""
        from backtide.analysis import plot_rolling_sharpe

        rng = np.random.default_rng(0)
        equity = list(np.cumprod(1 + rng.normal(0.001, 0.01, 200)) * 1_000)
        run = _run_result("S1", equity)
        fig = plot_rolling_sharpe([run], window=30, display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1


class TestPlotPnlHistogram:
    """Tests for plot_pnl_histogram."""

    def test_one_histogram_per_run(self):
        """Renders one histogram trace per run, named after the strategy."""
        from backtide.analysis import plot_pnl_histogram

        a = _make_run_with_trades("A")
        b = _make_run_with_trades("B", pnls=(10.0, 20.0, -5.0))
        fig = plot_pnl_histogram([a, b], display=None)
        assert len(fig.data) == 2
        assert {t.name for t in fig.data} == {"A", "B"}

    def test_empty_input(self):
        """An empty runs list should raise a ValueError."""
        from backtide.analysis import plot_pnl_histogram

        with pytest.raises(ValueError, match="cannot be empty"):
            plot_pnl_histogram([], display=None)


class TestPlotTradePnl:
    """Tests for plot_trade_pnl."""

    def test_one_scatter_per_run(self):
        """Renders one scatter trace per run in marker mode."""
        from backtide.analysis import plot_trade_pnl

        a = _make_run_with_trades("A")
        b = _make_run_with_trades("B")
        fig = plot_trade_pnl([a, b], display=None)
        assert len(fig.data) == 2
        for trace in fig.data:
            assert trace.mode == "markers"


class TestPlotTradeDuration:
    """Tests for plot_trade_duration."""

    def test_returns_figure(self):
        """Builds a duration-histogram trace from the run's trades."""
        from backtide.analysis import plot_trade_duration

        a = _make_run_with_trades("A")
        fig = plot_trade_duration([a], display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 1

    def test_no_trades(self):
        """Produces a blank figure when the run has no trades."""
        from backtide.analysis import plot_trade_duration

        empty = _run_result("Empty", [10_000.0])
        fig = plot_trade_duration([empty], display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 0


class TestPlotPositionSize:
    """Tests for plot_position_size."""

    def test_one_trace_per_symbol(self):
        """Reconstructs one cumulative-position trace per traded symbol."""
        from backtide.analysis import plot_position_size

        run = _make_run_with_trades("S1")
        fig = plot_position_size(run, display=None)
        # Two symbols traded (AAPL, MSFT).
        names = {t.name for t in fig.data}
        assert "AAPL" in names
        assert "MSFT" in names

    def test_no_orders(self):
        """Returns a blank figure when the run has no filled orders."""
        from backtide.analysis import plot_position_size

        run = _run_result("Empty", [10_000.0])
        fig = plot_position_size(run, display=None)
        assert isinstance(fig, go.Figure)
        # No trace traces, just the y=0 reference line annotation/shape.
        assert all(t.mode != "lines" or t.name == "" for t in fig.data) or len(fig.data) == 0


class TestMaeMfe:
    """Tests for plot_mae_mfe."""

    def test_uses_query_bars_and_classifies_winners(self, monkeypatch):
        """Uses query_bars to compute MAE/MFE and splits winners from losers."""
        from backtide.analysis import plot_mae_mfe

        # Synthetic bar data covering both trades' time windows.
        run = _make_run_with_trades("S1", pnls=(50.0, -30.0))
        bars = pd.DataFrame(
            {
                "open_ts": [t.entry_ts for t in run.trades] + [t.exit_ts for t in run.trades],
                "interval": ["1d", "1d", "1d", "1d"],
                "high": [110.0, 105.0, 110.0, 105.0],
                "low": [95.0, 90.0, 95.0, 90.0],
            }
        )
        monkeypatch.setattr("backtide.analysis.mae_mfe.query_bars", lambda **_: bars)
        fig = plot_mae_mfe(run, display=None)
        names = {t.name for t in fig.data}
        # Diagonal reference is rendered as a layout shape.
        assert fig.layout.shapes is not None
        assert len(fig.layout.shapes) >= 1
        assert "Winners" in names
        assert "Losers" in names

    def test_no_trades(self):
        """Returns an empty figure when the run has no trades."""
        from backtide.analysis import plot_mae_mfe

        empty = _run_result("Empty", [10_000.0])
        fig = plot_mae_mfe(empty, display=None)
        assert isinstance(fig, go.Figure)


class TestPlotPriceWithStrategyRun:
    """Tests for entry/exit marker overlays on plot_price."""

    def test_overlays_trade_markers(self, daily_bars):
        """Overlays entry/exit markers on top of the price chart."""
        run = _make_run_with_trades("S1", pnls=(10.0, -5.0))
        # All trades on AAPL, which is the only symbol in `daily_bars`.
        # Re-anchor trade timestamps inside the bar window.
        ts_first = int(daily_bars["open_ts"].iloc[0])
        for i, t in enumerate(run.trades):
            t.entry_ts = ts_first + i * 86_400
            t.exit_ts = ts_first + (i + 1) * 86_400

        fig = plot_price(daily_bars, run=run, display=None)
        names = {t.name for t in fig.data}
        # At minimum, we expect long/short entry and win/loss exit traces.
        assert any(n.startswith(("Long entry", "Short entry")) for n in names)
        assert any(n.startswith("Exit") for n in names)

    def test_without_strategy_run_unchanged(self, daily_bars):
        """Baseline plot_price behaviour is unchanged when strategy_run is None."""
        # Baseline behaviour: only the price line trace.
        fig = plot_price(daily_bars, display=None)
        assert len(fig.data) == 1


# ─────────────────────────────────────────────────────────────────────────────
# Coverage - early-return / edge-case branches
# ─────────────────────────────────────────────────────────────────────────────


class TestEmptyRunsRaises:
    """Every plot_* accepting a runs list raises ValueError when given an empty list."""

    @pytest.mark.parametrize(
        "module",
        [
            "rolling_returns",
            "rolling_sharpe",
            "trade_duration",
            "trade_pnl",
            "cash_holdings",
        ],
    )
    def test_empty_runs_raises(self, module):
        """An empty `runs` list raises a ValueError mentioning emptiness."""
        plot_fn = getattr(
            __import__(f"backtide.analysis.{module}", fromlist=["x"]),
            f"plot_{module}",
        )
        with pytest.raises(ValueError, match="cannot be empty"):
            plot_fn([], display=None)


class TestBenchmarkAndEmptySkips:
    """Plots silently skip benchmark or empty-trade/equity runs."""

    def test_cash_holdings_skips_benchmark(self):
        """Benchmark runs are skipped (no trace)."""
        run = _run_result("Bench", [10_000.0, 10_100.0], is_benchmark=True)
        fig = plot_cash_holdings([run], display=None)
        assert len(fig.data) == 0

    def test_cash_holdings_skips_empty_equity_curve(self):
        """Runs with an empty equity_curve are silently skipped."""
        run = _run_result("Empty", [])
        fig = plot_cash_holdings([run], display=None)
        assert len(fig.data) == 0

    def test_cash_holdings_invalid_currency_falls_back_to_code(self):
        """Unknown currency codes degrade gracefully (fall back to the raw code)."""
        run = _run_result("S1", [10_000.0, 10_100.0], base_currency="USD")
        # Override the cash bucket to use an unparseable code.
        for sample in run.equity_curve:
            sample.cash = {"XXX": 100.0}
        fig = plot_cash_holdings([run], display=None)
        # Y-axis label uses the raw code, not a Currency symbol.
        assert "XXX" in fig.layout.yaxis.title.text

    def test_pnl_histogram_skips_benchmark(self):
        """`plot_pnl_histogram` skips benchmark runs (no trace)."""
        from backtide.analysis import plot_pnl_histogram

        bench = _run_result("B", [10_000.0, 10_100.0], is_benchmark=True)
        # Use a non-benchmark run for the legend axis to render.
        active = _make_run_with_trades("S1")
        fig = plot_pnl_histogram([bench, active], display=None)
        assert {t.name for t in fig.data} == {"S1"}

    def test_trade_pnl_skips_benchmark(self):
        """`plot_trade_pnl` skips benchmark runs."""
        from backtide.analysis import plot_trade_pnl

        bench = _run_result("B", [10_000.0, 10_100.0], is_benchmark=True)
        active = _make_run_with_trades("S1")
        fig = plot_trade_pnl([bench, active], display=None)
        assert {t.name for t in fig.data} == {"S1"}

    def test_trade_duration_skips_benchmark(self):
        """`plot_trade_duration` skips benchmark runs."""
        from backtide.analysis import plot_trade_duration

        bench = _run_result("B", [10_000.0, 10_100.0], is_benchmark=True)
        active = _make_run_with_trades("S1")
        fig = plot_trade_duration([bench, active], display=None)
        # Only the active strategy is rendered.
        assert len(fig.data) == 1

    def test_pnl_skips_all_zero_equity(self):
        """`plot_pnl` skips runs whose equity curve is all zero."""
        run = _run_result("Zero", [0.0, 0.0, 0.0])
        fig = plot_pnl([run], drawdown=False, display=None)
        assert len(fig.data) == 0

    def test_pnl_benchmark_uses_dashed_line(self):
        """Benchmark runs render with the dashed reference style."""
        bench = _run_result("Bench", [10_000.0, 10_100.0], is_benchmark=True)
        fig = plot_pnl([bench], drawdown=False, display=None)
        # Benchmark line is dashed.
        assert fig.data[0].line.dash == "dash"


class TestRollingPlotsEdgeCases:
    """Edge-case coverage for rolling_returns / rolling_sharpe."""

    def test_rolling_sharpe_skips_short_runs(self):
        """Runs shorter than `window` produce no trace."""
        from backtide.analysis import plot_rolling_sharpe

        short = _run_result("S1", [100.0, 101.0, 102.0])
        fig = plot_rolling_sharpe([short], window=30, display=None)
        assert len(fig.data) == 0

    def test_rolling_sharpe_renders_benchmark(self):
        """Benchmark runs render with the BENCHMARK_LINE style."""
        from backtide.analysis import plot_rolling_sharpe

        rng = np.random.default_rng(1)
        equity = list(np.cumprod(1 + rng.normal(0.001, 0.01, 60)) * 1_000)
        bench = _run_result("Bench", equity, is_benchmark=True)
        fig = plot_rolling_sharpe([bench], window=20, display=None)
        assert len(fig.data) == 1
        # Benchmark line uses the dashed reference style.
        assert fig.data[0].line.dash == "dash"

    def test_rolling_returns_renders_benchmark(self):
        """Benchmark runs render with the BENCHMARK_LINE style on rolling_returns."""
        from backtide.analysis import plot_rolling_returns

        bench = _run_result("Bench", [100.0 + i for i in range(50)], is_benchmark=True)
        fig = plot_rolling_returns([bench], window=10, display=None)
        assert len(fig.data) == 1
        assert fig.data[0].line.dash == "dash"


class TestTradeDurationUnits:
    """Auto-unit selection in plot_trade_duration."""

    def test_auto_picks_days_for_long_trades(self):
        """Trades lasting weeks default to the `days` unit."""
        from backtide.analysis import plot_trade_duration

        # All trades last 10 days → auto-pick "days".
        base = 1_700_000_000
        pnls = (1.0, -1.0, 1.0)
        trades = [
            _StubTrade(
                symbol="AAPL",
                entry_ts=base + i * 86_400 * 30,
                exit_ts=base + i * 86_400 * 30 + 10 * 86_400,
                entry_price=100.0,
                exit_price=101.0,
                quantity=1,
                pnl=p,
            )
            for i, p in enumerate(pnls)
        ]
        run = _run_result("S1", [10_000.0, 10_001.0, 10_000.0], trades=trades)
        fig = plot_trade_duration([run], unit="auto", display=None)
        assert "days" in fig.layout.xaxis.title.text.lower()

    def test_auto_picks_hours_for_intraday(self):
        """Trades a few hours long pick the `hours` unit."""
        from backtide.analysis import plot_trade_duration

        base = 1_700_000_000
        trades = [
            _StubTrade(
                symbol="AAPL",
                entry_ts=base + i * 3_600 * 8,
                exit_ts=base + i * 3_600 * 8 + 4 * 3_600,
                entry_price=100.0,
                exit_price=101.0,
                quantity=1,
                pnl=1.0,
            )
            for i in range(3)
        ]
        run = _run_result("S1", [10_000.0] * 3, trades=trades)
        fig = plot_trade_duration([run], unit="auto", display=None)
        assert "hours" in fig.layout.xaxis.title.text.lower()


class TestReturnsEmptyData:
    """Coverage for empty-data branches in plot_returns."""

    def test_empty_symbol_fallback(self):
        """When no symbols produce returns, a blank figure is returned."""
        # Use the default price_col ("adj_close") so the column check passes.
        empty = pd.DataFrame({"symbol": [], "dt": [], "close": []})
        fig = plot_returns(empty, display=None)
        assert isinstance(fig, go.Figure)
        # Empty data → no histogram traces.
        assert len(fig.data) == 0


class TestPositionSizeFiltering:
    """`plot_position_size` respects the `symbols` filter."""

    def test_symbols_filter(self):
        """When `symbols=['AAPL']`, MSFT trades are excluded."""
        from backtide.analysis import plot_position_size

        run = _make_run_with_trades("S1")
        fig = plot_position_size(run, symbols=["AAPL"], display=None)
        names = {t.name for t in fig.data}
        assert "AAPL" in names
        assert "MSFT" not in names


class TestMaeMfeEdgeCases:
    """Coverage for the symbols filter and empty-bar window branch."""

    def test_symbols_filter_excludes_other_trades(self, monkeypatch):
        """The `symbols` argument filters which trades are analysed."""
        from backtide.analysis import plot_mae_mfe

        run = _make_run_with_trades("S1", pnls=(50.0, -30.0))
        # Synthetic bars for the trade window.
        bars = pd.DataFrame(
            {
                "open_ts": [t.entry_ts for t in run.trades] + [t.exit_ts for t in run.trades],
                "interval": ["1d", "1d", "1d", "1d"],
                "high": [110.0, 105.0, 110.0, 105.0],
                "low": [95.0, 90.0, 95.0, 90.0],
            }
        )
        monkeypatch.setattr("backtide.analysis.mae_mfe.query_bars", lambda **_: bars)
        # All trades are on AAPL — filtering to MSFT yields no marks.
        fig = plot_mae_mfe(run, symbols=["MSFT"], display=None)
        # Both winner & loser traces have empty x.
        for trace in fig.data:
            if trace.name in {"Winners", "Losers"}:
                assert len(trace.x) == 0

    def test_empty_bar_window_skips_trade(self, monkeypatch):
        """Trades whose bar window is empty are silently skipped."""
        from backtide.analysis import plot_mae_mfe

        run = _make_run_with_trades("S1", pnls=(50.0,))
        # Bars exist but outside the trade time window.
        far_ts = run.trades[0].entry_ts + 10 * 86_400
        bars = pd.DataFrame(
            {
                "open_ts": [far_ts, far_ts + 1],
                "interval": ["1d", "1d"],
                "high": [110.0, 105.0],
                "low": [95.0, 90.0],
            }
        )
        monkeypatch.setattr("backtide.analysis.mae_mfe.query_bars", lambda **_: bars)
        fig = plot_mae_mfe(run, display=None)
        # No win/loss markers — trade skipped because window was empty.
        for trace in fig.data:
            if trace.name in {"Winners", "Losers"}:
                assert len(trace.x) == 0


# ─────────────────────────────────────────────────────────────────────────────
# Analysis utils — _resolve_runs_currency / _get_currency_symbol fallbacks
# ─────────────────────────────────────────────────────────────────────────────


class TestResolveRunsCurrency:
    """Tests for the `_resolve_runs_currency` helper."""

    def test_single_currency_returns_it(self):
        """Same base currency across runs → returns it."""
        from backtide.analysis.utils import _resolve_runs_currency

        a = _run_result("A", [100.0], base_currency="USD")
        b = _run_result("B", [200.0], base_currency="USD")
        assert _resolve_runs_currency([a, b]) == Currency.USD

    def test_mixed_currencies_returns_none(self):
        """Mixed base currencies → returns None."""
        from backtide.analysis.utils import _resolve_runs_currency

        a = _run_result("A", [100.0], base_currency="USD")
        b = _run_result("B", [200.0], base_currency="EUR")
        assert _resolve_runs_currency([a, b]) is None


class TestGetCurrencySymbolFallback:
    """`_get_currency_symbol` returns None for unknown codes."""

    def test_unknown_code_returns_none(self):
        """Unknown codes fall through to None (ValueError caught)."""
        df = pd.DataFrame({"currency": ["XYZ", "XYZ"]})
        assert _get_currency_symbol(df) is None
