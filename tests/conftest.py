"""Backtide.

Author: Mavs
Description: Shared fixtures for the test suite.

"""

import pytest

from backtide.config import Config, DataConfig, set_config


@pytest.fixture(scope="session", autouse=True)
def _init_config(tmp_path_factory):
    """Set a deterministic config for all tests.

    Uses a temporary directory for storage so tests never touch the real DB.
    Uses yahoo as crypto provider since binance doesn't allow requests from CI.

    """
    set_config(
        Config(
            data=DataConfig(
                storage_path=str(tmp_path_factory.mktemp("test_backtide_storage")),
                providers={"crypto": "yahoo"},
            ),
        )
    )
