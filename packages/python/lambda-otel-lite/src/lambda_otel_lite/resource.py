"""
Resource creation utilities for lambda-otel-lite.

This module provides functions for creating and customizing Resource instances
with AWS Lambda-specific attributes.
"""

import os

from opentelemetry.sdk.resources import Resource

from .constants import Defaults, EnvVars, ResourceAttributes
from .logger import create_logger

# Setup logging
logger = create_logger("resource")


def get_lambda_resource(custom_resource: Resource | None = None) -> Resource:
    """Create a Resource instance with AWS Lambda attributes and OTEL environment variables.

    This function combines AWS Lambda environment attributes with any OTEL resource attributes
    specified via environment variables (OTEL_RESOURCE_ATTRIBUTES and OTEL_SERVICE_NAME).

    Resource attributes for configuration values are only set when the corresponding
    environment variables are explicitly set, following the pattern:
    1. Environment variables (recorded as resource attributes only when explicitly set)
    2. Constructor parameters (not recorded as resource attributes)
    3. Default values (not recorded as resource attributes)

    Returns:
        Resource instance with AWS Lambda and OTEL environment attributes
    """
    # Start with Lambda attributes
    attributes: dict[str, str | int | float | bool] = {"cloud.provider": "aws"}

    def parse_numeric_env(key: str, env_var: str) -> None:
        """Parse numeric environment variable and set attribute only if explicitly set."""
        env_value = os.environ.get(env_var)
        if env_value is not None:
            try:
                attributes[key] = int(env_value)
            except ValueError:
                logger.warn(
                    "Invalid numeric value for %s: %s, attribute not set",
                    key,
                    env_value,
                )

    def parse_memory_value(key: str, value: str | None, default: str) -> None:
        """Parse memory value from MB to bytes."""
        try:
            attributes[key] = int(value or default) * 1024 * 1024  # Convert MB to bytes
        except ValueError:
            logger.warn("Invalid memory value for %s: %s", key, value)

    # Map environment variables to attribute names
    env_mappings = {
        "AWS_REGION": "cloud.region",
        "AWS_LAMBDA_FUNCTION_NAME": "faas.name",
        "AWS_LAMBDA_FUNCTION_VERSION": "faas.version",
        "AWS_LAMBDA_LOG_STREAM_NAME": "faas.instance",
        "AWS_LAMBDA_FUNCTION_MEMORY_SIZE": "faas.max_memory",
    }

    # Add attributes only if they exist in environment
    for env_var, attr_name in env_mappings.items():
        if value := os.environ.get(env_var):
            if attr_name == "faas.max_memory":
                parse_memory_value(attr_name, value, "128")
            else:
                attributes[attr_name] = value

    # Add service name (guaranteed to have a value)
    service_name = os.environ.get(
        EnvVars.SERVICE_NAME,
        os.environ.get("AWS_LAMBDA_FUNCTION_NAME", Defaults.SERVICE_NAME),
    )
    attributes["service.name"] = service_name

    # Add telemetry configuration attributes only when environment variables are set
    processor_mode_env = os.environ.get(EnvVars.PROCESSOR_MODE)
    if processor_mode_env is not None:
        attributes[ResourceAttributes.PROCESSOR_MODE] = processor_mode_env

    # Parse numeric configuration values - only set if explicitly in environment
    parse_numeric_env(ResourceAttributes.QUEUE_SIZE, EnvVars.QUEUE_SIZE)
    parse_numeric_env(ResourceAttributes.BATCH_SIZE, EnvVars.BATCH_SIZE)
    parse_numeric_env(ResourceAttributes.COMPRESSION_LEVEL, EnvVars.COMPRESSION_LEVEL)

    # OTEL_RESOURCE_ATTRIBUTES are automatically parsed by the Resource create method
    # Create resource and merge with custom resource if provided
    resource = Resource(attributes)

    if custom_resource:
        # Merge in reverse order so custom resource takes precedence
        resource = resource.merge(custom_resource)

    final_resource = Resource.create().merge(resource)
    return final_resource
