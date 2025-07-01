// myotel/spanEvent.ts
import { trace, SpanAttributes, Span, TimeInput } from "@opentelemetry/api";

export enum Level { TRACE = 1, DEBUG = 5, INFO = 9, WARN = 13, ERROR = 17 }

const levelText: Record<number, string> = {
  1: "TRACE", 5: "DEBUG", 9: "INFO", 13: "WARN", 17: "ERROR"
};

const minLevel: Level = (() => {
  const raw = (process.env.EVENTS_LOG_LEVEL ?? "TRACE").toUpperCase();
  return (Level as any)[raw] ?? Level.TRACE;
})();

export function spanEvent(
  name: string,
  body: string,
  level: Level = Level.INFO,
  attrs: SpanAttributes = {},
  span?: Span,
  timestamp?: TimeInput
): void {
  if (level < minLevel) return;

  span ??= trace.getActiveSpan();
  if (!span?.isRecording()) return;

  span.addEvent(
    name,
    {
      ...attrs,
      "event.severity_text": levelText[level],
      "event.severity_number": level,
      "event.body": body,
    },
    timestamp
  );
}
