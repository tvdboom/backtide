"""Type stubs for `backtide.core.metrics` (auto-generated)."""

__all__ = ["MetricDefinition", "list_builtin_metrics"]

class MetricDefinition:
    """Metadata describing a built-in performance metric.

    Attributes
    ----------
    key : str
        Stable key stored in experiment results.

    name : str
        Human-readable display name.

    description : str
        Short explanation of the metric.

    percentage : bool
        Whether the value is a fractional percentage.

    higher_is_better : bool
        Whether larger values rank ahead of smaller values.

    """

    description: str
    higher_is_better: bool
    key: str
    name: str
    percentage: bool

    def __eq__(self, value, /):
        ...
    def __ge__(self, value, /):
        ...
    def __getstate__(self, /):
        ...
    def __gt__(self, value, /):
        ...
    def __init__(self, /, *args, **kwargs):
        ...
    def __le__(self, value, /):
        ...
    def __lt__(self, value, /):
        ...
    def __ne__(self, value, /):
        ...
    def __new__(cls, *args, **kwargs):
        ...
    def __repr__(self, /):
        ...
    def __str__(self, /):
        ...

def list_builtin_metrics() -> list[MetricDefinition]:
    """Return metadata for all built-in Rust metrics.

    Returns
    -------
    list[[MetricDefinition]]
        Stable definitions used by experiment configuration and result displays.

    """
