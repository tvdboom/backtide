"""Backtide.

Author: Mavs
Description: Analysis and plotting functionalities for backtide.

"""

from backtide.analysis.candlestick import plot_candlestick
from backtide.analysis.correlation import plot_correlation
from backtide.analysis.dividends import plot_dividends
from backtide.analysis.drawdown import plot_drawdown
from backtide.analysis.mae_mfe import plot_mae_mfe
from backtide.analysis.pnl import plot_pnl
from backtide.analysis.pnl_histogram import plot_pnl_histogram
from backtide.analysis.position_size import plot_position_size
from backtide.analysis.price import plot_price
from backtide.analysis.returns import plot_returns
from backtide.analysis.rolling_returns import plot_rolling_returns
from backtide.analysis.rolling_sharpe import plot_rolling_sharpe
from backtide.analysis.seasonality import plot_seasonality
from backtide.analysis.trade_duration import plot_trade_duration
from backtide.analysis.trade_pnl import plot_trade_pnl
from backtide.analysis.volatility import plot_volatility
from backtide.analysis.volume import plot_volume
from backtide.analysis.vwap import plot_vwap
from backtide.core.analysis import compute_statistics
