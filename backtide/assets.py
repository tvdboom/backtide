"""Backtide.

Author: Mavs
Description: Functions to retrieve available tickers.

"""

from dataclasses import dataclass
from enum import Enum
from functools import cache

from pytickersymbols.indices_data import INDICES


INDEX_CURRENCIES = {
    "AEX": "EUR",
    "BEL 20": "EUR",
    "CAC_40": "EUR",
    "CAC Mid 60": "EUR",
    "DAX": "EUR",
    "DOW JONES": "USD",
    "EURO STOXX 50": "EUR",
    "FTSE 100": "GBP",
    "IBEX 35": "EUR",
    "MDAX": "EUR",
    "NASDAQ 100": "USD",
    "NIKKEI 225": "JPY",
    "OMX Helsinki 25": "EUR",
    "OMX Stockholm 30": "SEK",
    "S&P 100": "USD",
    "S&P 500": "USD",
    "S&P 600": "USD",
    "SDAX": "EUR",
    "Switzerland 20": "CHF",
    "TecDAX": "EUR",
}


@dataclass
class Asset:
    """Represents a financial asset.

    Attributes
    ----------
    name : str
        The full name of the asset (e.g., "Apple Inc.").

    symbol : str
        The market identifier of the asset. For exchange-traded assets this
        is a ticker, for others a symbol (e.g., "AAPL" or "BTC").

    index : str
        Index or exchange this asset belongs to.

    currency : str
        The currency in which the asset is denominated (e.g., "USD", "EUR").

    """

    name: str
    symbol: str
    index: str
    currency: str


class AssetType(Enum):
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

    def identifier(self) -> str:
        """Return the name for the asset's market identifier."""
        match self:
            case AssetType.STOCKS | AssetType.ETF:
                return "ticker"
            case _:
                return "symbol"

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
            case _:
                raise ValueError(f"Invalid asset type: {self}")

    @cache
    def list_preloaded(self) -> list[Asset]:
        """Return the list of preloaded assets.

        - Stocks load the primary listings of the companies in major indices.

        """
        seen: list[str] = []
        result: list[Asset] = []

        match self:
            case AssetType.STOCKS:
                for index, data in INDICES.items():
                    for company in data["companies"]:
                        # Get the symbol from the primary listing
                        symbol = company.get("symbol")
                        if symbol and symbol not in seen:
                            seen.append(symbol)
                            result.append(
                                Asset(
                                    name=company["name"],
                                    symbol=symbol,
                                    index=index,
                                    currency=INDEX_CURRENCIES[index],
                                ),
                            )
            case _:
                raise ValueError(f"Invalid asset type: {self}")

        return result
