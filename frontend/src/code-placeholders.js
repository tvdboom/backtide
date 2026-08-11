export function strategyCodePlaceholder(dataframeClass = 'pd.DataFrame') {
  return `from backtide.strategies import BaseStrategy
from backtide.backtest import Order


class MyStrategy(BaseStrategy):
    def evaluate(self, data, portfolio, state, indicators):
        """Evaluate the strategy and return orders.

        Parameters
        ----------
        data : ${dataframeClass}
            Historical OHLCV data with columns 'symbol', 'open', 'high',
            'low', 'close', 'adj_close' 'volume'.

        portfolio : Portfolio
            Current portfolio holdings (cash and positions).

        state : State
            Current simulation state.

        indicators: ${dataframeClass} | None
            Indicators calculated on the historical data. None if no
            indicators were selected.

        Returns
        -------
        list[Order]
            Orders to place this tick.

        """
        orders = []

        # ── Write your logic here ────────────────────────



        # ───────────────────────────────────────────────────

        return orders


MyStrategy()`
}

export function indicatorCodePlaceholder(dataframeClass = 'pd.DataFrame') {
  return `from backtide.indicators import BaseIndicator


class MyIndicator(BaseIndicator):
    def compute(self, data):
        """Compute the indicator values.

        Parameters
        ----------
        data : ${dataframeClass}
            Historical OHLCV data with columns 'symbol', 'open', 'high',
            'low', 'close', 'adj_close' 'volume'.

        Returns
        -------
        ${dataframeClass}
            The computed values. For multi-output indicators (e.g., upper
            and lower bounds), return a 2d structure.

        """
        # ── Write your logic here ────────────────────────



        # ───────────────────────────────────────────────────

        return result


MyIndicator()`
}

export function metricCodePlaceholder(dataframeClass = 'pd.DataFrame') {
  return `from backtide.metrics import BaseMetric


class MyMetric(BaseMetric):
    def compute(self, equity_curve, trades):
        """Compute the metric value.

        Parameters
        ----------
        equity_curve : ${dataframeClass}
            Historical equity, return, and drawdown values.

        trades : ${dataframeClass}
            Completed trades and their profit and loss.

        Returns
        -------
        float
            The metric value.

        """
        # Write your logic here



        return result


MyMetric()`
}
