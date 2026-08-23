"""Backtide.

Author: Mavs
Description: Storage queries and re-exports of `backtide.core.storage`.

"""

from __future__ import annotations

import json
from typing import TYPE_CHECKING

from backtide.core.storage import (
    _append_live_session_event,
    _delete_live_session,
    _query_live_session,
    _query_live_session_events,
    _query_live_session_warmup,
    _query_live_sessions,
    _write_live_session,
    _write_live_session_warmup,
    delete_experiment,
    delete_symbols,
    query_bars,
    query_bars_summary,
    query_dividends,
    query_experiments,
    query_instruments,
    query_strategy_runs,
)

if TYPE_CHECKING:
    from backtide.backtest.study import StudyResult


def query_study(study_id: str) -> StudyResult | None:
    """Return a persisted study.

    Parameters
    ----------
    study_id : str
        Persisted study identifier.

    Returns
    -------
    [StudyResult] | None
        Stored study, or `None` when the experiment is a regular backtest.

    See Also
    --------
    - backtide.backtest.study:Study
    - backtide.storage:query_experiments

    Examples
    --------
    ```pycon
    from backtide.storage import query_study

    study = query_study("my-study-id")
    if study is not None:
        print(study.best_candidate)
    ```

    """
    from backtide.backtest.study import StudyResult, _result_path

    path = _result_path(study_id)
    if not path.is_file():
        return None
    value = json.loads(path.read_text(encoding="utf-8"))
    if not isinstance(value, dict):
        raise ValueError("The study result must contain a JSON object.")
    return StudyResult.from_dict(value)
