"""Type stubs for `backtide.core.live` (auto-generated)."""

__all__ = [
    "LiveMarketFeed",
    "MarketUpdate",
    "PaperFill",
    "PaperTradingConfig",
    "PaperTradingSession",
    "PaperTradingSnapshot",
    "PaperTradingUpdate",
    "collect_market_updates",
    "provider_live_support",
]

from backtide.core.backtest import Order, OrderStatus, Portfolio
from backtide.core.data import Currency

class LiveMarketFeed:
    """Reusable, cancellable exchange market-data collector.

    The feed retains a healthy WebSocket across bounded `collect` calls and
    retries disconnects with exponential backoff. Call `cancel` safely from
    another Python thread; cancellation latency is at most 250 ms and closes
    the retained socket. A later call requires `reset`.

    """

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def cancel(self):
        """Request cancellation of an in-progress `collect` call."""
    def collect(self, max_events=1, timeout_seconds=30.0):
        """Collect up to `max_events`, retrying transient disconnects."""
    def is_cancelled(self):
        """Whether cancellation has been requested."""
    def reset(self):
        """Clear cancellation before intentionally reusing this feed."""

class MarketUpdate:
    """A candle received from a live market-data connection.

    `is_final` is `true` only when the provider has closed the candle, or when
    Backtide observed the next candle and can therefore finalize the prior one.

    Attributes
    ----------
    provider : str
        Lowercase provider identifier, or `"mock"` for replay data.

    symbol : str
        Canonical provider-independent symbol.

    interval : str
        Canonical interval string.

    open_ts : int
        Candle-open Unix timestamp in seconds.

    close_ts : int
        Candle-close Unix timestamp in seconds.

    open : float
        Opening price in quote-currency units.

    high : float
        Highest price in quote-currency units.

    low : float
        Lowest price in quote-currency units.

    close : float
        Latest or final closing price in quote-currency units.

    volume : float
        Traded volume in base-asset units.

    n_trades : int | None
        Provider-reported trade count when available.

    is_final : bool
        Whether no further updates are expected for this candle.

    received_ts : int
        Local receipt Unix timestamp in seconds.

    """

    close: float
    close_ts: int
    high: float
    interval: str
    is_final: bool
    low: float
    n_trades: int | None
    open: float
    open_ts: int
    provider: str
    received_ts: int
    symbol: str
    volume: float

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class PaperFill:
    """Result of matching one paper order.

    Attributes
    ----------
    order : [Order]
        Submitted order after any sizer resolution.

    timestamp : int
        Fill, cancellation, or rejection Unix timestamp in seconds.

    status : [OrderStatus]
        Terminal order status.

    fill_price : float | None
        Executed quote-currency price, or `None` when not filled.

    commission : float
        Fee charged in the paper account's base currency.

    realized_pnl : float | None
        Change in realized PnL from this fill, net of its commission.

    reason : str
        Human-readable matching or rejection reason.

    """

    commission: float
    fill_price: float | None
    order: Order
    realized_pnl: float | None
    reason: str
    status: OrderStatus
    timestamp: int

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class PaperTradingConfig:
    """Configuration for a paper-trading session.

    Attributes
    ----------
    initial_cash : float, default=100000
        Starting cash balance in `base_currency`.

    base_currency : [Currency], default=Currency.USD
        Accounting currency for cash, fills, and equity.

    commission_pct : float, default=0
        Percentage commission charged on every fill (for example, `0.1`
        means 0.1%).

    commission_fixed : float, default=0
        Fixed commission charged on every fill.

    slippage : float, default=0
        Percentage slippage applied to fills.

    allow_short : bool, default=False
        Whether fills may create a negative position.

    allow_margin : bool, default=False
        Whether fills may create a negative cash balance.

    trade_on_partial : bool, default=False
        Whether strategy and order processing runs on incomplete candles.
        Keeping the default avoids repeated decisions on the same candle.

    max_history : int, default=10000
        Maximum bars retained per symbol for strategy evaluation.

    """

    allow_margin: bool
    allow_short: bool
    base_currency: Currency
    commission_fixed: float
    commission_pct: float
    initial_cash: float
    max_history: int
    slippage: float
    trade_on_partial: bool

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class PaperTradingSession:
    """A stateful paper-trading account with optional strategy evaluation.

    Parameters
    ----------
    config : [PaperTradingConfig] | None, default=None
        Execution, fee, and risk settings. Uses defaults when omitted.

    strategy : BaseStrategy | None, default=None
        Existing built-in or custom strategy. Its `evaluate` method runs after
        resting orders are matched on each processable candle. Explicit orders
        can also be passed to `on_bar`.

    """

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...
    def on_bar(self, market, orders=None) -> PaperTradingUpdate:
        """Process a live or replayed candle.

        Parameters
        ----------
        market : [MarketUpdate]
            Provider-normalized candle update.

        orders : list[[Order]] | None, default=None
            Explicit orders to submit after resting orders are matched. Orders
            returned by the configured strategy are appended automatically.

        Returns
        -------
        [PaperTradingUpdate]
            Fills plus a complete mark-to-market account snapshot.

        """
    def snapshot(self):
        """Return the current account state without processing a candle."""

class PaperTradingSnapshot:
    """Mark-to-market snapshot of a paper-trading account.

    Attributes
    ----------
    portfolio : [Portfolio]
        Cash, positions, and currently resting orders.

    latest_prices : dict[str, float]
        Latest valid close per canonical symbol.

    equity : float
        Cash plus positions marked to `latest_prices`.

    realized_pnl : float
        Cumulative realized PnL net of commissions.

    unrealized_pnl : float
        Open-position PnL marked to `latest_prices`.

    processed_bars : int
        Number of updates that triggered matching or strategy evaluation.

    """

    equity: float
    latest_prices: dict[str, float]
    portfolio: Portfolio
    processed_bars: int
    realized_pnl: float
    unrealized_pnl: float

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

class PaperTradingUpdate:
    """State transition produced after processing a market update.

    Attributes
    ----------
    market : [MarketUpdate]
        Market update supplied by the caller.

    fills : list[[PaperFill]]
        Orders filled, canceled, or rejected during this transition.

    snapshot : [PaperTradingSnapshot]
        Account state after this transition.

    orders_submitted : int
        Number of explicit and strategy orders submitted on this update.

    processed : bool
        Whether this update was new, valid, and eligible for trading.

    """

    fills: list[PaperFill]
    market: MarketUpdate
    orders_submitted: int
    processed: bool
    snapshot: PaperTradingSnapshot

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __hash__(self, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

def collect_market_updates(
    provider,
    symbols,
    interval=...,
    max_events=1,
    timeout_seconds=30.0,
    include_partial=True,
):
    """Collect a finite batch from an exchange WebSocket.

    Yahoo Finance is intentionally rejected because it has no official live
    market-data WebSocket. A timeout returns the updates collected so far.

    """

def provider_live_support(provider, interval=...):
    """Report live WebSocket support without opening a connection.

    Returns `(supported, explanation)`. Coinbase supports five-minute candles
    only; Yahoo has no supported WebSocket feed.

    """
