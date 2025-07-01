# myotel/span_event.py
from __future__ import annotations
import os
from enum import IntEnum
from datetime import datetime
from typing import Mapping, Any
from opentelemetry import trace

class Level(IntEnum):
    TRACE = 1; DEBUG = 5; INFO = 9; WARN = 13; ERROR = 17

_LEVEL_TEXT = {l: l.name for l in Level}

def _env_min_level() -> Level:
    value = os.getenv("EVENTS_LOG_LEVEL", "INFO").upper()
    return Level.__members__.get(value, Level.INFO)

_MIN_LEVEL: Level = _env_min_level()        # evaluated once at import

def span_event(
    name: str,                                 # short descriptive title
    body: str,                                 # long/extended text
    level: Level = Level.INFO,
    attrs: Mapping[str, Any] | None = None,
    span=None,
    ts_ns: int | None = None,
) -> None:
    """Emit a span event if `level` >= EVENTS_LOG_LEVEL."""
    if level < _MIN_LEVEL:
        return

    span = span or trace.get_current_span()
    if not span.is_recording():
        return

    span.add_event(
        name,
        {
            "event.severity_text": _LEVEL_TEXT[level],
            "event.severity_number": int(level),
            "event.body": body,
            **(attrs or {}),
        },
        ts_ns or int(datetime.utcnow().timestamp() * 1e9),
    )
