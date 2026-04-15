"""Type stubs for `backtide.core.utils` (auto-generated)."""

__all__ = ["clear_cache", "init_logging"]

def clear_cache():
    """Clears/invalidates all cache stored by the engine.

    See Also
    --------
    - backtide.utils:init_logging

    """

def init_logging(log_level):
    """Initialize the global logging subscriber.

    The logging level can only be set before it's used anywhere, so call this
    function at the start of the process. If logging was already initialized
    this results in a no-op.

    Parameters
    ----------
    log_level : str | [LogLevel]
        Minimum tracing log level. Choose from: "error", "warn", "info",
       "debug".

    See Also
    --------
    - backtide.utils:clear_cache

    """
