"""
Configuration utilities for lambda-otel-lite.

This module provides helper functions for reading and validating environment variables
with proper precedence.
"""

import os
from typing import Callable, Optional, TypeVar

from .logger import create_logger

logger = create_logger("config")

T = TypeVar("T")


def get_bool_env(
    name: str, config_value: Optional[bool] = None, default_value: bool = False
) -> bool:
    """Get a boolean value from an environment variable with proper precedence.

    Only "true" and "false" (case-insensitive) are accepted as valid values.
    If the environment variable is set but invalid, a warning is logged and
    the fallback value is used.

    Precedence order:
    1. Environment variable (if set and valid)
    2. Programmatic configuration (if provided)
    3. Default value

    Args:
        name: Name of the environment variable
        config_value: Optional programmatic configuration value
        default_value: Default value if neither env var nor config is set

    Returns:
        The resolved boolean value
    """
    env_value = os.getenv(name)

    # If environment variable is set and not empty
    if env_value is not None:
        value = env_value.strip().lower()
        if value == "true":
            return True
        if value == "false":
            return False

        # Invalid environment variable value - log warning and continue
        if value:
            logger.warn(
                f"Invalid boolean for {name}: {env_value}. Must be 'true' or 'false'. Using fallback."
            )

    # Use config value if provided, otherwise default
    return config_value if config_value is not None else default_value


def get_int_env(
    name: str,
    config_value: Optional[int] = None,
    default_value: int = 0,
    validator: Optional[Callable[[int], bool]] = None,
) -> int:
    """Get an integer value from an environment variable with proper precedence.

    If the environment variable is set but invalid, a warning is logged and
    the fallback value is used.

    Precedence order:
    1. Environment variable (if set and valid)
    2. Programmatic configuration (if provided)
    3. Default value

    Args:
        name: Name of the environment variable
        config_value: Optional programmatic configuration value
        default_value: Default value if neither env var nor config is set
        validator: Optional function to validate the parsed number

    Returns:
        The resolved integer value
    """
    env_value = os.getenv(name)

    # If environment variable is set and not empty
    if env_value is not None:
        try:
            parsed_value = int(env_value.strip())

            # If a validator is provided, check the value
            if validator and not validator(parsed_value):
                logger.warn(
                    f"Invalid value for {name}: {env_value}. Failed validation. Using fallback."
                )
            else:
                return parsed_value
        except ValueError:
            logger.warn(
                f"Invalid numeric value for {name}: {env_value}. Using fallback."
            )

    # Use config value if provided, otherwise default
    return config_value if config_value is not None else default_value


def get_str_env(
    name: str,
    config_value: Optional[str] = None,
    default_value: str = "",
    validator: Optional[Callable[[str], bool]] = None,
) -> str:
    """Get a string value from an environment variable with proper precedence.

    Precedence order:
    1. Environment variable (if set and not empty)
    2. Programmatic configuration (if provided)
    3. Default value

    Args:
        name: Name of the environment variable
        config_value: Optional programmatic configuration value
        default_value: Default value if neither env var nor config is set
        validator: Optional function to validate the string

    Returns:
        The resolved string value
    """
    env_value = os.getenv(name)

    # If environment variable is set and not empty
    if env_value is not None and env_value.strip() != "":
        value = env_value.strip()

        # If a validator is provided, check the value
        if validator and not validator(value):
            logger.warn(
                f"Invalid value for {name}: {value}. Failed validation. Using fallback."
            )
        else:
            return value

    # Use config value if provided, otherwise default
    return config_value if config_value is not None else default_value
