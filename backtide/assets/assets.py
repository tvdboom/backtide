"""Backtide.

Author: Mavs
Description: Functions to retrieve available tickers.

"""

import asyncio
from dataclasses import dataclass
from functools import cache
from typing import cast

from pytickersymbols.indices_data import INDICES

from backtide.assets.crypto import fetch_binance_assets
from backtide.assets.currency import CURRENCIES, INDEX_CURRENCIES, Currency
from backtide.assets.etf import ETFS
from backtide.assets.forex import FOREX
from backtide.utils.enum import CaseInsensitiveEnum
from backtide.utils.types import IndexData


@dataclass(frozen=True)
class Asset:
    """Represents a financial asset.

    Attributes
    ----------
    name : str
        The full name of the asset (e.g., "Apple Inc.").

    symbol : str
        The market identifier of the asset. For exchange-traded assets this
        is a ticker, for others a symbol (e.g., "AAPL" or "BTC").

    currency : Currency
        The currency in which the asset is denominated.

    """

    name: str
    symbol: str
    currency: Currency


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
        seen: list[str] = []
        result: dict[str, Asset] = {}

        indices = cast(dict[str, IndexData], INDICES)

        match self:
            case AssetType.STOCKS:
                for index, data in indices.items():
                    currency = INDEX_CURRENCIES[index]

                    for company in data["companies"]:
                        name = company["name"]

                        # Get all available symbols (some companies have multiple listings)
                        if symbols := company.get("symbols"):
                            symbols = [s.get("yahoo") for s in symbols]
                        else:
                            symbols = [company.get("symbol")]

                        # Select only primary listing. Choose the ticker with period
                        # if currency != USD (US listings have no exchange code, while others do)
                        p_symbol: str | None = None
                        for symbol in symbols:
                            if symbol:
                                if currency == "USD" and "." not in symbol:
                                    p_symbol = symbol
                                    break
                                elif currency != "USD" and "." in symbol:
                                    p_symbol = symbol
                                    break

                        if p_symbol and p_symbol not in seen:
                            seen.append(p_symbol)
                            result[p_symbol] = Asset(name, symbol=p_symbol, currency=currency)

            case AssetType.FOREX:
                result = {
                    f"{c1}{c2}=X": Asset(
                        name=f"{c1}/{c2}",
                        symbol=f"{c1}{c2}=X",
                        currency=CURRENCIES[c2],
                    )
                    for c1, c2 in FOREX
                }

            case AssetType.ETF:
                result = {
                    etf["ticker"]: Asset(
                        name=etf["name"],
                        symbol=etf["ticker"],
                        currency=CURRENCIES[etf["currency"]],
                    )
                    for etf in ETFS
                }

            case AssetType.CRYPTO:
                result = asyncio.run(fetch_binance_assets())

        return result
