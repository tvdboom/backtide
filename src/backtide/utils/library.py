"""Backtide.

Author: Mavs
Description: Shared helpers for custom extension libraries.

"""

from __future__ import annotations

import ast
from collections.abc import Callable
import logging
import os
from pathlib import Path
import tempfile
from typing import Any, TypeVar

import cloudpickle

T = TypeVar("T")


def _build_custom_instance(
    code: str,
    *,
    filename: str,
    expected_type: type[T],
    missing_expression: str,
    type_error: Callable[[Any], str],
) -> T:
    """Execute custom source once and return its final expression."""
    tree = ast.parse(code)
    if not tree.body or not isinstance(tree.body[-1], ast.Expr):
        raise ValueError(missing_expression)

    namespace: dict[str, Any] = {}
    definitions = ast.Module(body=tree.body[:-1], type_ignores=tree.type_ignores)
    exec(compile(definitions, filename, "exec"), namespace)
    expression = ast.Expression(body=tree.body[-1].value)
    instance = eval(compile(expression, filename, "eval"), namespace)
    if not isinstance(instance, expected_type):
        raise TypeError(type_error(instance))

    # Source inspection is unreliable after unpickling, so retain the input.
    instance._source_code = code
    return instance


def _load_pickles(
    folder: Path,
    *,
    logger: logging.Logger,
    item_name: str,
    restore: Callable[[Any], T] | None = None,
    remove_empty: bool = False,
) -> dict[str, T]:
    """Load a sorted directory of pickles, isolating corrupt entries."""
    values: dict[str, T] = {}
    restore_value: Callable[[Any], T] = restore or (lambda value: value)
    for file in sorted(folder.glob("*.pkl")):
        try:
            if remove_empty and file.stat().st_size == 0:
                file.unlink()
                continue
            with file.open("rb") as stream:
                values[file.stem] = restore_value(cloudpickle.load(stream))
        except Exception as exc:  # noqa: BLE001
            logger.warning("Failed to load %s %s: %s", item_name, file.stem, exc)
    return values


def _save_pickle(
    value: Any,
    folder: Path,
    name: str,
    *,
    temporary_prefix: str,
) -> None:
    """Persist one value with an fsynced atomic file replacement."""
    folder.mkdir(parents=True, exist_ok=True)
    target = folder / f"{name}.pkl"
    temporary: Path | None = None
    try:
        with tempfile.NamedTemporaryFile(
            mode="wb",
            dir=folder,
            prefix=temporary_prefix,
            suffix=".tmp",
            delete=False,
        ) as stream:
            temporary = Path(stream.name)
            cloudpickle.dump(value, stream)
            stream.flush()
            os.fsync(stream.fileno())
        temporary.replace(target)
    finally:
        if temporary is not None:
            temporary.unlink(missing_ok=True)
