"""Backtide.

Author: Mavs
Description: Functions to retrieve available tickers.

"""

from functools import cache
from typing import Self

from backtide.assets.forex import FOREX
from backtide.core import Asset, MarketData
from backtide.utils.constants import MAX_PRELOADED_ASSETS
from backtide.utils.enum import CaseInsensitiveEnum


class AssetType(CaseInsensitiveEnum):
    """Financial asset types.

    Attributes
    ----------
    STOCKS : str
        Stocks or equities traded on an exchange (e.g., AAPL, TSLA).

    FOREX : str
        Foreign exchange currency pairs (e.g., EUR/USD, GBP/JPY).

    ETF : str
        Exchange-traded funds representing a basket of assets (e.g., VOO, QQQ).

    CRYPTO : str
        Cryptocurrencies or digital assets (e.g., BTC, ETH).

    """

    STOCKS = "Stocks"
    FOREX = "Forex"
    ETF = "ETF"
    CRYPTO = "Crypto"

    @classmethod
    def names(cls) -> list[str]:
        """Get the list of asset types."""
        return [asset.value for asset in cls]

    @classmethod
    def default(cls) -> Self:
        """Get the default asset type."""
        return AssetType.STOCKS

    def icon(self) -> str:
        """Return the material icon of the asset."""
        match self:
            case AssetType.STOCKS:
                return ":material/candlestick_chart:"
            case AssetType.FOREX:
                return ":material/currency_exchange:"
            case AssetType.ETF:
                return ":material/account_balance:"
            case AssetType.CRYPTO:
                return ":material/currency_bitcoin:"

    @cache  # noqa: B019
    def list_assets(self) -> dict[str, Asset]:
        """Return the preloaded assets.

        - Stocks: Primary listings of the companies in major indices.
        - Forex: Frequently traded currency pairs loaded from `currency.py`.
        - ETF: Frequently traded ETFs/funds loaded from `etf.py`.
        - Crypto: All active spot symbols retrieved from the exchange's API.

        Returns
        -------
        dict[str, Asset]
            Preloaded symbol-asset key-value pairs for this asset type.

        """
        market_data = MarketData()

        match self:
            case AssetType.STOCKS:
                result = market_data.list_stocks(MAX_PRELOADED_ASSETS)
            case AssetType.FOREX:
                result = [
                    Asset(
                        name=f"{c1}/{c2}",
                        symbol=f"{c1}{c2}=X" if c1 != "USD" else f"{c2}=X",
                        currency=c2,
                    )
                    for c1, c2 in FOREX
                ]
            case AssetType.ETF:
                result = market_data.list_etf(MAX_PRELOADED_ASSETS)
            case AssetType.CRYPTO:
                result = market_data.list_crypto(MAX_PRELOADED_ASSETS)

        return {x.symbol: x for x in result}
