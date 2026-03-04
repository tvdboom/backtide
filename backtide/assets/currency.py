"""Backtide.

Author: Mavs
Description: Currency listings.

"""

from dataclasses import dataclass


@dataclass(frozen=True)
class Currency:
    """Represents a fiat or digital currency.

    Attributes
    ----------
    name : str
        ISO 4217 alphabetic code for fiat currencies (e.g., "USD"), or the
        ticker symbol for cryptocurrencies (e.g., "BTC").

    full_name : str
        The full human-readable name of the currency (e.g., "United States dollar").

    decimals : int
        Number of decimal places the currency operates in. For fiat currencies
        this follows ISO 4217 (e.g., 2 for USD, 0 for JPY). For cryptocurrencies
        this reflects the network's native precision (e.g., 8 for BTC, 6 for XRP).

    code : int or None
        ISO 4217 numeric code assigned to the currency (e.g., 840 for USD).
        None for cryptocurrencies.

    country : str or None
        The country or region that issues the currency (e.g., "United States").
        None for cryptocurrencies.

    flag : str or None
        Flag emoji representing the currency's country or region (e.g., "🇺🇸").
        None for cryptocurrencies.

    """

    name: str
    full_name: str
    decimals: int
    code: int | None = None
    country: str | None = None
    flag: str | None = None

    def __eq__(self, other: object) -> bool:
        """Also allow comparison with string."""
        if isinstance(other, str):
            return self.name.lower() == other.lower()

        return self is other

    def __hash__(self) -> int:
        """Implement since __eq__ overrides it."""
        return hash(self.name)

    def __repr__(self) -> str:
        """Represent the currency as the name."""
        return self.name


# Major currencies
CURRENCIES: dict[str, Currency] = {
    c.name: c
    for c in [
        Currency("AUD", "Australian dollar", 2, 36, "Australia", "🇦🇺"),
        Currency("BRL", "Brazilian real", 2, 986, "Brazil", "🇧🇷"),
        Currency("CAD", "Canadian dollar", 2, 124, "Canada", "🇨🇦"),
        Currency("CHF", "Swiss franc", 2, 756, "Switzerland", "🇨🇭"),
        Currency("CNY", "Chinese yuan", 2, 156, "China", "🇨🇳"),
        Currency("CZK", "Czech koruna", 2, 203, "Czechia", "🇨🇿"),
        Currency("DKK", "Danish krone", 2, 208, "Denmark", "🇩🇰"),
        Currency("EUR", "Euro", 2, 978, "Europe", "🇪🇺"),
        Currency("GBP", "Pound sterling", 2, 826, "United Kingdom", "🇬🇧"),
        Currency("HKD", "Hong Kong dollar", 2, 344, "Hong Kong", "🇭🇰"),
        Currency("HUF", "Hungarian forint", 2, 348, "Hungary", "🇭🇺"),
        Currency("IDR", "Indonesian rupiah", 2, 360, "Indonesia", "🇮🇩"),
        Currency("INR", "Indian rupee", 2, 356, "India", "🇮🇳"),
        Currency("JPY", "Japanese yen", 0, 392, "Japan", "🇯🇵"),
        Currency("KRW", "South Korean won", 0, 410, "South Korea", "🇰🇷"),
        Currency("MXN", "Mexican peso", 2, 484, "Mexico", "🇲🇽"),
        Currency("MYR", "Malaysian ringgit", 2, 458, "Malaysia", "🇲🇾"),
        Currency("NOK", "Norwegian krone", 2, 578, "Norway", "🇳🇴"),
        Currency("NZD", "New Zealand dollar", 2, 554, "New Zealand", "🇳🇿"),
        Currency("PHP", "Philippine peso", 2, 608, "Philippines", "🇵🇭"),
        Currency("PLN", "Polish złoty", 2, 985, "Poland", "🇵🇱"),
        Currency("RUB", "Russian rouble", 2, 643, "Russia", "🇷🇺"),
        Currency("SAR", "Saudi riyal", 2, 682, "Saudi Arabia", "🇸🇦"),
        Currency("SEK", "Swedish krona", 2, 752, "Sweden", "🇸🇪"),
        Currency("SGD", "Singapore dollar", 2, 702, "Singapore", "🇸🇬"),
        Currency("THB", "Thai baht", 2, 764, "Thailand", "🇹🇭"),
        Currency("TRY", "Turkish lira", 2, 949, "Turkey", "🇹🇷"),
        Currency("TWD", "New Taiwan dollar", 2, 901, "Taiwan", "🇹🇼"),
        Currency("USD", "United States dollar", 2, 840, "United States", "🇺🇸"),
        Currency("ZAR", "South African rand", 2, 710, "South Africa", "🇿🇦"),
    ]
}


# Major indices and the currencies in which they are denominated
INDEX_CURRENCIES: dict[str, Currency] = {
    "AEX": CURRENCIES["EUR"],
    "BEL 20": CURRENCIES["EUR"],
    "CAC_40": CURRENCIES["EUR"],
    "CAC Mid 60": CURRENCIES["EUR"],
    "DAX": CURRENCIES["EUR"],
    "DOW JONES": CURRENCIES["USD"],
    "EURO STOXX 50": CURRENCIES["EUR"],
    "FTSE 100": CURRENCIES["GBP"],
    "IBEX 35": CURRENCIES["EUR"],
    "MDAX": CURRENCIES["EUR"],
    "NASDAQ 100": CURRENCIES["USD"],
    "NIKKEI 225": CURRENCIES["JPY"],
    "OMX Helsinki 25": CURRENCIES["EUR"],
    "OMX Stockholm 30": CURRENCIES["SEK"],
    "S&P 100": CURRENCIES["USD"],
    "S&P 500": CURRENCIES["USD"],
    "S&P 600": CURRENCIES["USD"],
    "SDAX": CURRENCIES["EUR"],
    "Switzerland 20": CURRENCIES["CHF"],
    "TecDAX": CURRENCIES["EUR"],
}
