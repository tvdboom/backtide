from pathlib import Path
from backtide.core.provider import Provider
from backtide.core.currency import Currency

class DisplayConfig:
    date_format: str
    timezone: str
    logokit_api_key: str
    def __repr__(self) -> str: ...

class ProviderConfig:
    stocks: Provider
    etf: Provider
    forex: Provider
    crypto: Provider
    def __repr__(self) -> str: ...

class DataConfig:
    storage_path: Path
    providers: ProviderConfig
    def __repr__(self) -> str: ...

class Config:
    base_currency: Currency
    data: DataConfig
    display: DisplayConfig
    def __repr__(self) -> str: ...

def get_config() -> Config: ...
def load_config(path: str) -> Config: ...
def set_config(cfg: Config) -> None: ...
