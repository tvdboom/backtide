"""Backtide.

Author: Mavs
Description: Unit tests for the configuration frontend.

"""

import json

import pytest
import yaml

from backtide.config import (
    Config,
    DataConfig,
    DisplayConfig,
    GeneralConfig,
    get_config,
    load_config,
)

# ─────────────────────────────────────────────────────────────────────────────
# DisplayConfig
# ─────────────────────────────────────────────────────────────────────────────


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
        assert str(DisplayConfig()).startswith('DisplayConfig(data_backend="pandas"')

    def test_datetime_format(self):
        """datetime_format concatenates date and time formats."""
        dc = DisplayConfig(date_format="YYYY/MM/DD", time_format="HH:MM:SS")
        assert dc.datetime_format() == "YYYY/MM/DD HH:MM:SS"

    def test_port_default(self):
        """Default port is 8501."""
        assert DisplayConfig().port == 8501

    def test_custom_port(self):
        """Custom port is persisted."""
        assert DisplayConfig(port=9000).port == 9000

    def test_address_default_none(self):
        """Default address is None."""
        assert DisplayConfig().address is None

    def test_custom_address(self):
        """Custom address is persisted."""
        assert DisplayConfig(address="0.0.0.0").address == "0.0.0.0"

    def test_logokit_api_key_default_none(self):
        """Default logokit_api_key is None."""
        assert DisplayConfig().logokit_api_key is None

    def test_to_dict_contains_all_keys(self):
        """to_dict returns all expected keys."""
        d = DisplayConfig().to_dict()
        for key in ("date_format", "time_format", "timezone", "port", "address"):
            assert key in d


# ─────────────────────────────────────────────────────────────────────────────
# DataConfig
# ─────────────────────────────────────────────────────────────────────────────


class TestDataConfig:
    """Tests for the 'DataConfig' class."""

    def test_equality(self):
        """DataConfig equality behaves correctly."""
        assert DataConfig() == DataConfig()
        assert DataConfig(storage_path="/tmp/") != DataConfig()

    def test_repr(self):
        """__repr__ contains data configuration values."""
        assert str(DataConfig()).startswith('DataConfig(storage_path=".backtide"')

    def test_default_providers(self):
        """Default providers map every instrument type."""
        d = DataConfig().to_dict()
        assert "stocks" in d["providers"]
        assert "crypto" in d["providers"]

    @pytest.mark.parametrize("provider", ["yahoo", "kraken", "coinbase", "binance"])
    def test_valid_provider(self, provider):
        """Valid provider strings are accepted."""
        dc = DataConfig(providers={"crypto": provider})
        assert dc.to_dict()["providers"]["crypto"].lower() == provider

    def test_invalid_provider_raises(self):
        """Invalid provider raises ValueError."""
        with pytest.raises(ValueError, match=r".*Unknown provider.*"):
            DataConfig(providers={"crypto": "invalid"})


# ─────────────────────────────────────────────────────────────────────────────
# GeneralConfig
# ─────────────────────────────────────────────────────────────────────────────


class TestGeneralConfig:
    """Tests for the 'GeneralConfig' class."""

    def test_default_base_currency(self):
        """Default base currency is USD."""
        gc = GeneralConfig()
        assert str(gc.base_currency) == "USD"

    def test_custom_base_currency(self):
        """Custom base currency overrides default."""
        gc = GeneralConfig(base_currency="EUR")
        assert str(gc.base_currency) == "EUR"

    def test_to_dict(self):
        """to_dict returns all expected keys."""
        d = GeneralConfig().to_dict()
        assert "base_currency" in d
        assert "triangulation_strategy" in d
        assert "log_level" in d

    def test_repr(self):
        """__repr__ contains general configuration values."""
        r = repr(GeneralConfig())
        assert "GeneralConfig" in r
        assert "USD" in r

    def test_invalid_currency_raises(self):
        """Invalid base currency raises ValueError."""
        with pytest.raises(ValueError, match=r".*Unknown currency.*"):
            GeneralConfig(base_currency="invalid")


# ─────────────────────────────────────────────────────────────────────────────
# Config
# ─────────────────────────────────────────────────────────────────────────────


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

    def test_to_dict_sections(self):
        """to_dict returns all three sections."""
        d = Config().to_dict()
        assert "general" in d
        assert "data" in d
        assert "display" in d


# ─────────────────────────────────────────────────────────────────────────────
# load_config / get_config
# ─────────────────────────────────────────────────────────────────────────────


class TestLoadConfig:
    """Tests for the 'load_config' and 'get_config' functions."""

    def test_load_toml(self, tmp_path):
        """load_config parses a TOML file."""
        p = tmp_path / "backtide.config.toml"
        p.write_text('[general]\nbase_currency = "EUR"\n')
        cfg = load_config(str(p))
        assert str(cfg.general.base_currency) == "EUR"

    def test_load_yaml(self, tmp_path):
        """load_config parses a YAML file."""
        p = tmp_path / "backtide.config.yaml"
        p.write_text(yaml.dump({"general": {"base_currency": "GBP"}}))
        cfg = load_config(str(p))
        assert str(cfg.general.base_currency) == "GBP"

    def test_load_json(self, tmp_path):
        """load_config parses a JSON file."""
        p = tmp_path / "backtide.config.json"
        p.write_text(json.dumps({"general": {"base_currency": "CHF"}}))
        cfg = load_config(str(p))
        assert str(cfg.general.base_currency) == "CHF"

    def test_load_invalid_path_raises(self):
        """load_config raises on a nonexistent path."""
        with pytest.raises(RuntimeError, match=r".*I/O error.*"):
            load_config("/nonexistent/path/config.toml")

    def test_get_config_returns_config(self):
        """get_config returns a Config instance."""
        cfg = get_config()
        assert isinstance(cfg, Config)
