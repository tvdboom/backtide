"""Backtide.

Author: Mavs
Description: Unit tests for the configuration frontend.

"""

import pytest

from backtide.config import (
    Config,
    DisplayConfig,
    IngestionConfig,
    ProviderConfig,
)


def test_provider_custom():
    """Custom provider overrides default."""
    p = ProviderConfig(crypto="kraken")
    assert p.to_dict()["crypto"].lower() == "kraken"


def test_provider_equality():
    """ProviderConfig equality behaves correctly."""
    assert ProviderConfig() == ProviderConfig()
    assert ProviderConfig(crypto="kraken") != ProviderConfig()


def test_provider_repr():
    """__repr__ contains expected provider information."""
    r = repr(ProviderConfig())
    assert "ProviderConfig" in r
    assert "yahoo" in r.lower()
    assert "binance" in r.lower()


def test_display_custom():
    """Custom display configuration overrides defaults."""
    d = DisplayConfig(date_format="%Y", timezone="UTC")
    out = d.to_dict()

    assert out["date_format"] == "%Y"
    assert out["timezone"] == "UTC"


def test_display_equality():
    """DisplayConfig equality behaves correctly."""
    assert DisplayConfig() == DisplayConfig()
    assert DisplayConfig(timezone="UTC") != DisplayConfig()


def test_display_repr():
    """__repr__ contains display configuration values."""
    r = repr(DisplayConfig())
    assert "DisplayConfig" in r
    assert "%d-%m-%Y" in r


def test_ingestion_custom_path():
    """Custom storage path is applied correctly."""
    i = IngestionConfig(storage_path="/tmp/")
    assert "database.duckdb" in i.to_dict()["storage_path"]


def test_ingestion_equality():
    """IngestionConfig equality behaves correctly."""
    assert IngestionConfig() == IngestionConfig()
    assert IngestionConfig(storage_path="/tmp/") != IngestionConfig()


def test_ingestion_repr():
    """__repr__ contains ingestion configuration values."""
    r = repr(IngestionConfig())
    assert "IngestionConfig" in r
    assert "database.duckdb" in r


def test_config_custom():
    """Custom base currency overrides default."""
    c = Config(base_currency="USD")
    assert c.to_dict()["base_currency"].lower() == "usd"


def test_config_nested_override():
    """Nested configuration overrides propagate correctly."""
    providers = ProviderConfig(crypto="kraken")
    ingestion = IngestionConfig(providers=providers)
    c = Config(ingestion=ingestion)

    assert c.to_dict()["ingestion"]["providers"]["crypto"].lower() == "kraken"


def test_config_equality():
    """Config equality behaves correctly."""
    assert Config() == Config()
    assert Config(base_currency="USD") != Config()


def test_config_repr():
    """__repr__ contains top-level config sections."""
    r = repr(Config())
    assert "Config" in r
    assert "ingestion" in r.lower()
    assert "display" in r.lower()


def test_invalid_provider_raises():
    """Invalid provider raises ValueError."""
    with pytest.raises(ValueError, match=".*Invalid provider.*"):
        ProviderConfig(crypto="invalid")


def test_invalid_currency_raises():
    """Invalid base currency raises ValueError."""
    with pytest.raises(ValueError, match=".*Invalid base currency.*"):
        Config(base_currency="invalid")
