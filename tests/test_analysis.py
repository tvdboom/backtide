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
    ):
        self.strategy_name = name
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
) -> RunResult:
    """Build a `_StubRun` and expose it as a `RunResult` for type checkers."""
    return cast(RunResult, _StubRun(name, equity, start, trades, orders, base_currency))


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
            _run_result("Benchmark (SPY)", [10_000.0, 10_100.0]),
        ]
        fig = plot_pnl(runs, drawdown=False, display=None)
        bench = next(t for t in fig.data if t.name.startswith("Benchmark"))
        # `line.dash` is None for solid lines and "dash" for the benchmark.
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
        assert fig.layout.yaxis.title.text == "Cash holdings ($)"

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
        assert fig.layout.yaxis.title.text == "Cash holdings"


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
        """Returns an empty figure when no runs are passed."""
        from backtide.analysis import plot_pnl_histogram

        fig = plot_pnl_histogram([], display=None)
        assert isinstance(fig, go.Figure)
        assert len(fig.data) == 0


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


class TestPlotMaeMfe:
    """Tests for plot_mae_mfe."""

    def test_uses_query_bars_and_classifies_winners(self, monkeypatch):
        """Uses query_bars to compute MAE/MFE and splits winners from losers."""
        from backtide.analysis import plot_mae_mfe

        # Synthetic bar data covering both trades' time windows.
        run = _make_run_with_trades("S1", pnls=(50.0, -30.0))
        bars = pd.DataFrame(
            {
                "open_ts": [t.entry_ts for t in run.trades] + [t.exit_ts for t in run.trades],
                "high": [110.0, 105.0, 110.0, 105.0],
                "low": [95.0, 90.0, 95.0, 90.0],
            }
        )
        monkeypatch.setattr(
            "backtide.analysis.mae_mfe.query_bars",
            lambda **_: bars,
        )
        fig = plot_mae_mfe(run, display=None)
        names = {t.name for t in fig.data}
        # Diagonal reference line plus both winner and loser scatters.
        assert "MFE = MAE" in names
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
