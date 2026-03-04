"""Backtide.

Author: Mavs
Description: Case-insensitive enumerator.

"""

from enum import Enum
from typing import Self


class CaseInsensitiveEnum(Enum):
    """Enumerator that allows case-insensitive name access.

    After name access fails, the method `_missing_` is called automatically,
    which returns any member based on name matching.

    """

    def __repr__(self) -> str:
        """Represent the enum as the name."""
        return self.name

    @classmethod
    def _missing_(cls, value: object) -> Self:
        """Inherited by child for case-insensitive name access."""
        if isinstance(value, str):
            value = value.lower()
            for member in cls:
                if member.name.lower() == value:
                    return member

        raise ValueError(f"{cls.__name__} has no member {value}.")
