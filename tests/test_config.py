"""Backtide.

Author: Mavs
Description: Unit tests for the configuration frontend.

"""

import pytest

from backtide.config import Config, DataConfig, DisplayConfig


def test_display_custom():
    """Custom display configuration overrides defaults."""
    dc = DisplayConfig(date_format="%Y", timezone="UTC").to_dict()
    assert dc["date_format"] == "%Y"
    assert dc["timezone"] == "UTC"


def test_display_equality():
    """DisplayConfig equality behaves correctly."""
    assert DisplayConfig() == DisplayConfig()
    assert DisplayConfig(timezone="UTC") != DisplayConfig()


def test_display_repr():
    """__repr__ contains display configuration values."""
    assert str(DisplayConfig()).startswith('DisplayConfig(date_format="YYYY-MM-DD"')


def test_data_equality():
    """DataConfig equality behaves correctly."""
    assert DataConfig() == DataConfig()
    assert DataConfig(storage_path="/tmp/") != DataConfig()


def test_data_repr():
    """__repr__ contains data configuration values."""
    assert str(DataConfig()).startswith('DataConfig(storage_path=".backtide"')


def test_config_custom():
    """Custom base currency overrides default."""
    c = Config(base_currency="USD")
    assert c.to_dict()["base_currency"].lower() == "usd"


def test_config_nested_override():
    """Nested configuration overrides propagate correctly."""
    c = Config(data=DataConfig(providers={"crypto": "kraken"}))
    assert c.to_dict()["data"]["providers"]["crypto"].lower() == "kraken"


def test_config_equality():
    """Config equality behaves correctly."""
    assert Config() == Config()
    assert Config(base_currency="EUR") != Config()


def test_config_repr():
    """__repr__ contains top-level config sections."""
    assert str(Config()).startswith('Config(base_currency="USD"')


def test_invalid_provider_raises():
    """Invalid provider raises ValueError."""
    with pytest.raises(ValueError, match=".*Invalid provider.*"):
        DataConfig(providers={"crypto": "invalid"})


def test_invalid_currency_raises():
    """Invalid base currency raises ValueError."""
    with pytest.raises(ValueError, match=".*Invalid base currency.*"):
        Config(base_currency="invalid")
