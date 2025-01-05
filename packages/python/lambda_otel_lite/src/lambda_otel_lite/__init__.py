"""
lambda-otel-lite - Lightweight OpenTelemetry instrumentation for AWS Lambda.

This package provides a simple way to add OpenTelemetry instrumentation to AWS Lambda
functions with minimal overhead and configuration.
"""

import os
from enum import Enum

__version__ = "0.1.0"


class ProcessorMode(str, Enum):
    """Controls how spans are processed and exported.

    Inherits from str to make it JSON serializable and easier to use with env vars.

    Attributes:
        SYNC: Synchronous flush in handler thread. Best for development.
        ASYNC: Asynchronous flush via extension. Best for production.
        FINALIZE: Let processor handle flushing. Best with BatchSpanProcessor.
    """

    SYNC = "sync"
    ASYNC = "async"
    FINALIZE = "finalize"

    @classmethod
    def from_env(cls, env_var: str, default: "ProcessorMode | None" = None) -> "ProcessorMode":
        """Create ProcessorMode from environment variable.

        Args:
            env_var: Name of the environment variable to read
            default: Default mode if environment variable is not set

        Returns:
            ProcessorMode instance

        Raises:
            ValueError: If environment variable contains invalid mode
        """
        value = os.getenv(env_var, "").lower() or (default.value if default else "")
        try:
            return cls(value)
        except ValueError as err:
            raise ValueError(
                f"Invalid {env_var}: {value}. Must be one of: {', '.join(m.value for m in cls)}"
            ) from err


# Package exports
__all__ = [
    "ProcessorMode",
    "init_telemetry",  # Will be imported from telemetry.py
    "traced_handler",  # Will be imported from handler.py
]

# Import public API
from .handler import traced_handler  # noqa: E402
from .telemetry import init_telemetry  # noqa: E402
