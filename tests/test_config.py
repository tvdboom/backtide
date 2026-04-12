"""Backtide.

Author: Mavs
Description: Unit tests for the configuration frontend.

"""

import pytest

from backtide.config import Config, DataConfig, DisplayConfig, GeneralConfig


class TestDisplayConfig:
    """Tests for the 'DisplayConfig' class."""

    def test_custom(self):
        """Custom display configuration overrides defaults."""
        dc = DisplayConfig(date_format="%Y", timezone="UTC").to_dict()
        assert dc["date_format"] == "%Y"
        assert dc["timezone"] == "UTC"

    def test_equality(self):
        """DisplayConfig equality behaves correctly."""
        assert DisplayConfig() == DisplayConfig()
        assert DisplayConfig(timezone="UTC") != DisplayConfig()

    def test_repr(self):
        """__repr__ contains display configuration values."""
        assert str(DisplayConfig()).startswith('DisplayConfig(date_format="YYYY-MM-DD"')


class TestDataConfig:
    """Tests for the 'DataConfig' class."""

    def test_equality(self):
        """DataConfig equality behaves correctly."""
        assert DataConfig() == DataConfig()
        assert DataConfig(storage_path="/tmp/") != DataConfig()

    def test_repr(self):
        """__repr__ contains data configuration values."""
        assert str(DataConfig()).startswith('DataConfig(storage_path=".backtide"')


class TestConfig:
    """Tests for the 'Config' class."""

    def test_custom(self):
        """Custom base currency overrides default."""
        c = Config(GeneralConfig(base_currency="USD"))
        assert c.to_dict()["general"]["base_currency"].lower() == "usd"

    def test_nested_override(self):
        """Nested configuration overrides propagate correctly."""
        c = Config(data=DataConfig(providers={"crypto": "kraken"}))
        assert c.to_dict()["data"]["providers"]["crypto"].lower() == "kraken"

    def test_equality(self):
        """Config equality behaves correctly."""
        assert Config() == Config()
        assert Config(GeneralConfig(base_currency="EUR")) != Config()

    def test_repr(self):
        """__repr__ contains top-level config sections."""
        assert str(Config()).startswith('Config(general=GeneralConfig(base_currency="USD"')

    def test_invalid_provider_raises(self):
        """Invalid provider raises ValueError."""
        with pytest.raises(ValueError, match=r".*Unknown provider.*"):
            DataConfig(providers={"crypto": "invalid"})

    def test_invalid_currency_raises(self):
        """Invalid base currency raises ValueError."""
        with pytest.raises(ValueError, match=r".*Unknown currency.*"):
            Config(GeneralConfig(base_currency="invalid"))
