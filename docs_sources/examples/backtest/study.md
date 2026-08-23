# Run a study

This example evaluates nine constructor combinations on the full sample, excludes candidates with
too few trades or excessive drawdown, and validates the training-window winner on consecutive
one-year test windows.

The study contains one experiment for each parameter combination, plus temporary training and test
experiments when walk-forward validation is enabled. The strategy can be a saved Library name or a
runtime instance. A runtime custom strategy must keep its constructor values on same-named
attributes so Backtide can create an isolated instance for every candidate and fold.

```python
from backtide import DataExpConfig, ExperimentConfig, GeneralExpConfig, Study, WalkForwardConfig
from backtide.strategies import BaseStrategy


class CustomCrossover(BaseStrategy):
    def __init__(self, fast: int = 20, slow: int = 100):
        self.fast = fast
        self.slow = slow

    def evaluate(self, data, portfolio, state, indicators):
        # Replace this with the strategy's order logic.
        return []


config = ExperimentConfig(
    general=GeneralExpConfig(name="Custom crossover study"),
    data=DataExpConfig(
        symbols=["SPY"],
        interval="1d",
        start_date="2012-01-01",
        end_date="2025-12-31",
        full_history=False,
    ),
    metrics=["sharpe", "total_return", "max_dd", "n_trades"],
)

# The first metric is both the experiment headline and the study objective.

study = Study(
    config,
    strategy=CustomCrossover(),
    parameter_space={
        "fast": [10, 20, 30],
        "slow": [75, 100, 150],
    },
    min_trades=20,
    max_drawdown=0.25,
    walk_forward=WalkForwardConfig(
        training_days=3 * 365,
        test_days=365,
        step_days=365,
        anchored=False,
    ),
)
result = study.run()  # norun

print(result.study_id, result.best_candidate.parameters)  # norun
for candidate in sorted(result.candidates, key=lambda item: item.rank or 10_000):  # norun
    print(candidate.rank, candidate.parameters, candidate.metrics.get("sharpe"))  # norun

for fold in result.folds:  # norun
    print(  # norun
        fold.fold,  # norun
        fold.parameters,  # norun
        fold.training_objective,  # norun
        fold.test_objective,  # norun
    )  # norun
```

`max_drawdown=0.25` means a 25% drawdown magnitude. Temporary training and test experiments are
deleted after each fold; `result.study_id` identifies the persisted study. Reopen
its summary later with [`query_study`](../../api/storage/query_study.md).
