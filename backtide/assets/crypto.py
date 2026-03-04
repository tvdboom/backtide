"""Backtide.

Author: Mavs
Description: Cryptocurrency API.

"""

from typing import TYPE_CHECKING

import streamlit as st

from backtide.assets.currency import CURRENCIES, Currency
from backtide.utils.client import HttpClient


if TYPE_CHECKING:
    from backtide.assets.assets import Asset


# Major cryptocurrencies
CRYPTOS: dict[str, Currency] = {
    c.name: c
    for c in [
        Currency("AAVE", "Aave", 8),
        Currency("ADA", "Cardano", 6),
        Currency("ALGO", "Algorand", 6),
        Currency("APT", "Aptos", 8),
        Currency("ARB", "Arbitrum", 8),
        Currency("ATOM", "Cosmos", 6),
        Currency("AVAX", "Avalanche", 8),
        Currency("AXS", "Axie Infinity", 8),
        Currency("BCH", "Bitcoin Cash", 8),
        Currency("BNB", "BNB", 8),
        Currency("BTC", "Bitcoin", 8),
        Currency("CRO", "Cronos", 8),
        Currency("DAI", "Dai", 2),
        Currency("DOGE", "Dogecoin", 8),
        Currency("DOT", "Polkadot", 8),
        Currency("EOS", "EOS", 4),
        Currency("ETC", "Ethereum Classic", 8),
        Currency("ETH", "Ethereum", 8),
        Currency("FIL", "Filecoin", 8),
        Currency("FTM", "Fantom", 8),
        Currency("GRT", "The Graph", 8),
        Currency("HBAR", "Hedera", 8),
        Currency("ICP", "Internet Computer", 8),
        Currency("INJ", "Injective", 8),
        Currency("LINK", "Chainlink", 8),
        Currency("LTC", "Litecoin", 8),
        Currency("MANA", "Decentraland", 8),
        Currency("MATIC", "Polygon", 8),
        Currency("MKR", "Maker", 8),
        Currency("NEAR", "NEAR Protocol", 8),
        Currency("OP", "Optimism", 8),
        Currency("QNT", "Quant", 8),
        Currency("SAND", "The Sandbox", 8),
        Currency("SHIB", "Shiba Inu", 8),
        Currency("SOL", "Solana", 8),
        Currency("STX", "Stacks", 8),
        Currency("SUI", "Sui", 8),
        Currency("THETA", "Theta Network", 8),
        Currency("TRX", "TRON", 6),
        Currency("UNI", "Uniswap", 8),
        Currency("USDC", "USD Coin", 2),
        Currency("USDT", "Tether", 2),
        Currency("VET", "VeChain", 8),
        Currency("XLM", "Stellar", 7),
        Currency("XRP", "XRP", 6),
        Currency("XTZ", "Tezos", 6),
    ]
}


async def fetch_binance_assets() -> dict[str, Asset]:
    """Get the full list of actively traded spot symbols from Binance.

    Returns
    -------
    dict[str, Asset]
        Preloaded symbol-asset key-value pairs.

    """
    from backtide.assets.assets import Asset

    async with HttpClient() as client:
        try:
            response = await client.apiget("https://api.binance.com/api/v3/exchangeInfo")

            def extract_currency(quote: str) -> Currency:
                """Convert a quote value to a currency.

                Extract quote from predefined currencies or cryptos, else create
                a default.

                """
                if quote in CURRENCIES:
                    return CURRENCIES[quote]
                elif quote in CRYPTOS:
                    return CRYPTOS[quote]
                else:
                    return Currency(quote, full_name=quote, decimals=8)

            return {
                data["symbol"]: Asset(
                    name=data["symbol"],
                    symbol=data["symbol"],
                    currency=extract_currency(data["quoteAsset"]),
                )
                for data in response["symbols"]
                if data["status"] == "TRADING" and data["isSpotTradingAllowed"]
            }
        except Exception as ex:  # noqa: BLE001
            st.exception(ex)

    return {}
