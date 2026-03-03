"""Backtide.

Author: Mavs
Description: Functions to retrieve available tickers.

"""

import asyncio
from dataclasses import dataclass
from enum import Enum
from functools import cache
from itertools import product

from pytickersymbols.indices_data import INDICES

from backtide.assets.crypto import fetch_binance_assets
from backtide.assets.currency import CURRENCIES, INDEX_CURRENCIES
from backtide.assets.etf import ETFS


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

    currency : str
        The currency in which the asset is denominated (e.g., "USD", "EUR").

    """

    name: str
    symbol: str
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

    @cache
    def list_symbols(self) -> list[Asset]:
        """Return the preloaded symbols.

        - Stocks: Primary listings of the companies in major indices.
        - Forex: Frequently traded currency pairs loaded from `currency.py`.
        - ETF: Frequently traded ETFs/funds loaded from `etf.py`.
        - Crypto: All active spot symbols retrieved from the exchange's API.

        Returns
        -------
        list[Asset]
            Preloaded symbols for this asset type.

        """
        seen: list[str] = []
        result: list[Asset] = []

        match self:
            case AssetType.STOCKS:
                for index, data in INDICES.items():
                    curr = INDEX_CURRENCIES[index]

                    for company in data["companies"]:
                        name = company["name"]

                        # Get all available symbols (some companies have multiple listings)
                        if symbols := company.get("symbols"):
                            symbols = [s.get("yahoo") for s in symbols]
                        else:
                            symbols = [company.get("symbol")]

                        # Select only primary listing. Choose the ticker with period
                        # if currency != USD (US listings have no exchange code, while others do)
                        primary_symbol: str | None = None
                        for sym in symbols:
                            if sym:
                                if curr == "USD" and "." not in sym:
                                    primary_symbol = sym
                                    break
                                elif curr != "USD" and "." in sym:
                                    primary_symbol = sym
                                    break

                        if primary_symbol and primary_symbol not in seen:
                            seen.append(primary_symbol)
                            result.append(Asset(name=name, symbol=primary_symbol, currency=curr))

            case AssetType.FOREX:
                pairs = [(c1, c2) for c1, c2 in product(CURRENCIES, repeat=2) if c1 != c2]
                result = [
                    Asset(name=f"{c1}/{c2}", symbol=f"{c1}{c2}=X", currency=c2) for c1, c2 in pairs
                ]

            case AssetType.ETF:
                result = [
                    Asset(name=etf["name"], symbol=etf["ticker"], currency=etf["currency"])
                    for etf in ETFS
                ]

            case AssetType.CRYPTO:
                result = asyncio.run(fetch_binance_assets())

        return result
