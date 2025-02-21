"""
Logging utilities for lambda-otel-lite.

This module provides a consistent logging interface that respects Lambda's JSON
logging configuration while providing a simpler interface outside of Lambda.
"""

import logging
import os
from typing import Any


def _is_running_in_lambda() -> bool:
    """Check if we're running in AWS Lambda.

    Returns:
        True if running in Lambda, False otherwise
    """
    return bool(os.environ.get("AWS_LAMBDA_FUNCTION_NAME"))


class Logger:
    """Logger with level filtering and consistent prefixing."""

    def __init__(self, prefix: str = "lambda-otel-lite") -> None:
        """Initialize logger with prefix.

        Args:
            prefix: The prefix to add to all log messages
        """
        self._prefix = prefix
        self._log_level = (
            os.environ.get("AWS_LAMBDA_LOG_LEVEL")
            or os.environ.get("LOG_LEVEL")
            or "info"
        ).lower()

        # Create logger
        self._logger = logging.getLogger(prefix)
        self._logger.setLevel(logging.DEBUG)  # Let level filtering happen in methods

        # Only add handler if:
        # 1. We're not in Lambda (where logging is already configured)
        # 2. The logger doesn't already have handlers
        if not _is_running_in_lambda() and not self._logger.handlers:
            handler = logging.StreamHandler()
            handler.setFormatter(
                logging.Formatter(
                    "%(message)s"
                )  # Simple format, we'll add prefix in methods
            )
            self._logger.addHandler(handler)

    def _should_log(self, level: str) -> bool:
        """Check if we should log at this level.

        Args:
            level: The log level to check

        Returns:
            True if we should log at this level
        """
        if self._log_level == "none":
            return False
        if self._log_level == "error":
            return level == "error"
        if self._log_level == "warn":
            return level in ["error", "warn"]
        if self._log_level == "info":
            return level in ["error", "warn", "info"]
        if self._log_level == "debug":
            return True
        return True  # Default to logging if level is unknown

    def _format_message(self, args: tuple[Any, ...]) -> tuple[str, tuple[Any, ...]]:
        """Format message based on args.

        If first arg is a string with format specifiers, use it as a format string.
        Otherwise, join all args with spaces.

        Args:
            args: The arguments to format

        Returns:
            Tuple of (format_string, format_args)
        """
        if not args:
            return "", ()

        first_arg = args[0]
        # When in Lambda, don't add our own prefix as it will be added by Lambda's formatter
        prefix = "" if _is_running_in_lambda() else f"[{self._prefix}] "

        if isinstance(first_arg, str) and "%" in first_arg:
            return f"{prefix}{first_arg}", args[1:]
        return f"{prefix}%s", (" ".join(str(arg) for arg in args),)

    def debug(self, *args: Any) -> None:
        """Log at DEBUG level.

        Args:
            *args: Values to log. If first arg is a format string, remaining args are used for
              formatting. Otherwise, args are joined with spaces.
        """
        if self._should_log("debug"):
            fmt, fmt_args = self._format_message(args)
            self._logger.debug(fmt, *fmt_args)

    def info(self, *args: Any) -> None:
        """Log at INFO level.

        Args:
            *args: Values to log. If first arg is a format string, remaining args are used for
              formatting. Otherwise, args are joined with spaces.
        """
        if self._should_log("info"):
            fmt, fmt_args = self._format_message(args)
            self._logger.info(fmt, *fmt_args)

    def warn(self, *args: Any) -> None:
        """Log at WARN level.

        Args:
            *args: Values to log. If first arg is a format string, remaining args are used for
              formatting. Otherwise, args are joined with spaces.
        """
        if self._should_log("warn"):
            fmt, fmt_args = self._format_message(args)
            self._logger.warning(fmt, *fmt_args)

    def error(self, *args: Any) -> None:
        """Log at ERROR level.

        Args:
            *args: Values to log. If first arg is a format string, remaining args are used for
            formatting. Otherwise, args are joined with spaces.
        """
        if self._should_log("error"):
            fmt, fmt_args = self._format_message(args)
            self._logger.error(fmt, *fmt_args)


def create_logger(prefix: str) -> Logger:
    """Create a logger with a specific prefix.

    Args:
        prefix: The prefix to add to all log messages

    Returns:
        A logger instance with the specified prefix
    """
    return Logger(prefix)


# Default logger instance
logger = Logger()
