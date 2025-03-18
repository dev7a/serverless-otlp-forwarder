"""
OpenTelemetry Stdout Span Exporter

A span exporter that writes OpenTelemetry spans to stdout in OTLP format.
"""

from .constants import EnvVars, Defaults, ResourceAttributes
from .exporter import OTLPStdoutSpanExporter
from .version import VERSION

__version__ = VERSION
__all__ = ["OTLPStdoutSpanExporter", "EnvVars", "Defaults", "ResourceAttributes"]
