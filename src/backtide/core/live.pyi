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
    "list_live_instruments",
]

from backtide.core.backtest import Order, OrderStatus, Portfolio
from backtide.core.data import Currency, Instrument

class LiveMarketFeed:
    """Reusable, cancellable exchange market-data collector.

    The feed retains a healthy WebSocket across bounded `collect` calls and
    retries disconnects with exponential backoff. Call `cancel` safely from
    another Python thread; cancellation latency is at most 250 ms and closes
    the retained socket. A later call requires `reset`.

    Parameters
    ----------
    provider : str | Provider
        Exchange WebSocket provider.

    symbols : list[str]
        Provider symbols to subscribe to.

    interval : str | Interval, default="1m"
        Candle interval. Coinbase supports `"5m"` only.

    include_partial : bool, default=True
        Include updates for candles that have not closed yet.

    reconnect_attempts : int, default=5
        Maximum connection attempts for a collection batch.

    backoff_seconds : float, default=0.25
        Initial reconnect delay in seconds.

    See Also
    --------
    - backtide.live:collect_market_updates
    - backtide.live:PaperTradingSession

    Examples
    --------
    ```pycon
    from backtide.live import LiveMarketFeed

    feed = LiveMarketFeed("kraken", ["BTC-USD"], interval="1m")
    feed.cancel()
    print(feed.is_cancelled())
    ```

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
        """Request cancellation of an in-progress `collect` call.

        Examples
        --------
        ```pycon
        from backtide.live import LiveMarketFeed

        feed = LiveMarketFeed("kraken", ["BTC-USD"])
        feed.cancel()
        print(feed.is_cancelled())
        ```

        """
    def collect(self, max_events=1, timeout_seconds=30.0) -> list[MarketUpdate]:
        """Collect up to `max_events`, retrying transient disconnects.

        Parameters
        ----------
        max_events : int, default=1
            Maximum number of updates to return.

        timeout_seconds : float, default=30
            Maximum collection time in seconds.

        Returns
        -------
        list[[MarketUpdate]]
            Updates received before the event limit or timeout.

        Examples
        --------
        ```pycon
        from backtide.live import LiveMarketFeed

        feed = LiveMarketFeed("binance", ["BTC-USDT"])
        updates = feed.collect(max_events=10, timeout_seconds=5)  # norun
        ```

        """
    def is_cancelled(self) -> bool:
        """Whether cancellation has been requested.

        Returns
        -------
        bool
            `True` after `cancel` and `False` after construction or `reset`.

        Examples
        --------
        ```pycon
        from backtide.live import LiveMarketFeed

        feed = LiveMarketFeed("kraken", ["BTC-USD"])
        print(feed.is_cancelled())
        ```

        """
    def reset(self):
        """Clear cancellation before intentionally reusing this feed.

        Examples
        --------
        ```pycon
        from backtide.live import LiveMarketFeed

        feed = LiveMarketFeed("kraken", ["BTC-USD"])
        feed.cancel()
        feed.reset()
        print(feed.is_cancelled())
        ```

        """

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

    See Also
    --------
    - backtide.live:PaperTradingSession

    Examples
    --------
    ```pycon
    from backtide.live import MarketUpdate

    market = MarketUpdate(
        symbol="BTC-USD",
        interval="1m",
        open_ts=1_700_000_000,
        close_ts=1_700_000_060,
        open=100.0,
        high=102.0,
        low=99.0,
        close=101.0,
        volume=5.0,
    )
    print(market.close)
    ```

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

    See Also
    --------
    - backtide.live:PaperTradingUpdate

    Examples
    --------
    ```pycon
    from backtide.backtest import Order
    from backtide.live import MarketUpdate, PaperTradingSession

    market = MarketUpdate(
        "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
        100.0, 102.0, 99.0, 101.0,
    )
    fill = PaperTradingSession().on_bar(market, [Order("BTC-USD", 1.0)]).fills[0]
    print(fill.fill_price)
    ```

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

    See Also
    --------
    - backtide.live:PaperTradingSession

    Examples
    --------
    ```pycon
    from backtide.live import PaperTradingConfig

    config = PaperTradingConfig(
        initial_cash=25_000,
        commission_pct=0.1,
        slippage=0.05,
    )
    print(config.initial_cash)
    ```

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

    See Also
    --------
    - backtide.live:MarketUpdate
    - backtide.live:PaperTradingConfig

    Examples
    --------
    ```pycon
    from backtide.live import MarketUpdate, PaperTradingSession

    session = PaperTradingSession()
    update = session.on_bar(
        MarketUpdate(
            "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
            100.0, 102.0, 99.0, 101.0, volume=5.0,
        )
    )
    print(update.snapshot.equity)
    ```

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

        Examples
        --------
        ```pycon
        from backtide.backtest import Order
        from backtide.live import MarketUpdate, PaperTradingSession

        session = PaperTradingSession()
        market = MarketUpdate(
            "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
            100.0, 102.0, 99.0, 101.0,
        )
        update = session.on_bar(market, [Order("BTC-USD", 1.0)])
        print(update.processed)
        ```

        """
    def snapshot(self) -> PaperTradingSnapshot:
        """Return the current account state without processing a candle.

        Returns
        -------
        [PaperTradingSnapshot]
            Current cash, positions, prices, and profit-and-loss values.

        Examples
        --------
        ```pycon
        from backtide.live import PaperTradingSession

        snapshot = PaperTradingSession().snapshot()
        print(snapshot.equity)
        ```

        """

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

    See Also
    --------
    - backtide.live:PaperTradingSession

    Examples
    --------
    ```pycon
    from backtide.live import PaperTradingSession

    snapshot = PaperTradingSession().snapshot()
    print(snapshot.equity)
    print(snapshot.portfolio.positions)
    ```

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

    See Also
    --------
    - backtide.live:MarketUpdate
    - backtide.live:PaperTradingSession

    Examples
    --------
    ```pycon
    from backtide.live import MarketUpdate, PaperTradingSession

    market = MarketUpdate(
        "BTC-USD", "1m", 1_700_000_000, 1_700_000_060,
        100.0, 102.0, 99.0, 101.0,
    )
    update = PaperTradingSession().on_bar(market)
    print(update.processed)
    print(update.snapshot.equity)
    ```

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
    interval='1m',
    max_events=1,
    timeout_seconds=30.0,
    include_partial=True,
) -> list[MarketUpdate]:
    """Collect a finite batch from an exchange WebSocket.

    A timeout returns the updates collected so far.

    !!! warning "Yahoo Finance is not supported"
        Yahoo Finance is intentionally rejected because it does not provide an
        official live market-data WebSocket. Choose Binance, Coinbase, or Kraken.

    Parameters
    ----------
    provider : str | [Provider]
        Public WebSocket source. Use `"binance"`, `"coinbase"`, or `"kraken"`.
        `"yahoo"` is accepted by historical-data APIs but rejected here.

    symbols : list[str]
        One or more canonical market symbols to subscribe to, such as
        `"BTC-USDT"` for Binance or `"BTC-USD"` for Coinbase and Kraken.

    interval : str | [Interval], default="1m"
        Duration represented by each candle. Accepted strings are `"1m"`,
        `"5m"`, `"15m"`, `"30m"`, `"1h"`, `"4h"`, `"1d"`, and `"1w"` where
        supported by the provider. Coinbase live collection supports `"5m"` only.

    max_events : int, default=1
        Maximum number of updates to return across all subscribed symbols.

    timeout_seconds : float, default=30
        Maximum number of seconds to wait for the batch. When it expires, the
        function returns any updates already received, including an empty list.

    include_partial : bool, default=True
        Whether to include in-progress candle revisions. Set to `False` to
        receive only candles the provider has marked as final.

    Returns
    -------
    list[[MarketUpdate]]
        Updates received before the event limit or timeout.

    See Also
    --------
    - backtide.live:LiveMarketFeed
    - backtide.live:PaperTradingSession

    Examples
    --------
    ```pycon
    from backtide.live import collect_market_updates

    updates = collect_market_updates(  # norun
        "binance",
        ["BTC-USDT"],
        interval="1m",
        max_events=10,
        timeout_seconds=5,
    )
    ```

    """

def list_live_instruments(provider, limit=10000) -> list[Instrument]:
    """List the spot instruments available from a live WebSocket provider.

    Parameters
    ----------
    provider : str | [Provider]
        Live provider whose complete spot catalog should be returned. Yahoo
        Finance is rejected because it has no supported live WebSocket.

    limit : int, default=10000
        Maximum number of instruments to return.

    Returns
    -------
    list[[Instrument]]
        Canonical symbols and metadata reported by the selected provider.

    Examples
    --------
    ```pycon
    from backtide.live import list_live_instruments

    instruments = list_live_instruments("kraken", limit=100)
    print(instruments[0].symbol)
    ```

    """
