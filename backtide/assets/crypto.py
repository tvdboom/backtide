"""Backtide.

Author: Mavs
Description: Cryptocurrency API.

"""

from typing import TYPE_CHECKING

import streamlit as st

from backtide.utils.client import HttpClient


if TYPE_CHECKING:
    from backtide.assets.assets import Asset


async def fetch_binance_assets() -> list[Asset]:
    """Get the full list of Binance symbols as assets.

    Returns
    -------
    list[str]
        All actively traded spot symbols.

    """
    from backtide.assets.assets import Asset

    async with HttpClient() as client:
        try:
            response = await client.apiget("https://api.binance.com/api/v3/exchangeInfo")
            return [
                Asset(name=data["symbol"], symbol=data["symbol"], currency=data["quoteAsset"])
                for data in response["symbols"]
                if data["status"] == "TRADING" and data["isSpotTradingAllowed"]
            ]
        except Exception as ex:  # noqa: BLE001
            return st.exception(ex)
