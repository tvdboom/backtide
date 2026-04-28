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
    plot_correlation,
    plot_dividends,
    plot_drawdown,
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
        for col in ("ann_return", "ann_volatility", "sharpe_ratio", "max_drawdown", "win_rate"):
            assert col in result.columns, f"Missing column: {col}"

    def test_custom_price_col(self, daily_bars):
        """Accept a custom price column."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars, price_col="close"))
        assert len(result) == 1

    def test_max_drawdown_non_positive(self, daily_bars):
        """Max drawdown should be <= 0."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars))
        assert result.iloc[0]["max_drawdown"] <= 0

    def test_win_rate_in_range(self, daily_bars):
        """Win rate should be between 0 and 100."""
        result = cast(pd.DataFrame, compute_statistics(daily_bars))
        wr = result.iloc[0]["win_rate"]
        assert 0 <= wr <= 100
