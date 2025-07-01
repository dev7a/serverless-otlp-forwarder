"""
OpenTelemetry SQS processing conventions:

Batch (SQS event) span:
- kind: SpanKind.CONSUMER
- name: "<queue> process"
- faas.trigger: "pubsub"
- messaging.system: "aws_sqs"
- messaging.operation.name: "process"
- messaging.operation.type: "process"
- messaging.batch.message_count: number of messages in the batch
- messaging.destination.name: queue name
- messaging.destination.kind: "queue"
- carrier: uses X-Amzn-Trace-Id for context propagation

Per-message span:
- kind: SpanKind.CONSUMER
- name: "<queue> process"
- faas.trigger: "pubsub"
- messaging.system: "aws_sqs"
- messaging.operation.name: "process"
- messaging.operation.type: "process"
- messaging.destination.name: queue name
- messaging.destination.kind: "queue"
- messaging.message.id: SQS messageId
- links: extracted from messageAttributes or AWSTraceHeader

References:
- https://opentelemetry.io/docs/specs/semconv/messaging/messaging-spans/
- https://opentelemetry.io/docs/specs/semconv/faas/aws-lambda/
"""

import logging
from typing import List, Iterator
from contextlib import contextmanager

from opentelemetry import propagate, trace
from opentelemetry.propagators.aws import AwsXRayPropagator
from opentelemetry.trace import Link, SpanKind, Tracer, Span
from opentelemetry.instrumentation.boto3sqs import boto3sqs_getter
from lambda_otel_lite import SpanAttributes, TriggerType, default_extractor
import os

logger = logging.getLogger(__name__)

def sqs_event_extractor(event, context):
    """Extract span attributes from SQS events"""
    
    base = default_extractor(event, context)
    attributes = base.attributes.copy()
    
    
    # Add SQS specific attributes
    
    records = event.get("Records")
    queue_arn = records[0]["eventSourceARN"]
    queue_name = queue_arn.split(":")[-1]
    attributes["messaging.destination.name"] = queue_name
    
    return SpanAttributes(
        trigger=TriggerType.PUBSUB,
        kind=SpanKind.CONSUMER,
        span_name=f"{queue_name} process",
        attributes=base.attributes.copy() | {
            "faas.trigger": "pubsub",
            "messaging.system": "aws_sqs",
            "messaging.operation.name": "process",
            "messaging.operation.type": "process",
            "messaging.batch.message_count": len(records),
            "messaging.destination.name": queue_name,
            "messaging.destination.kind": "queue"
        },
        carrier={
            "X-Amzn-Trace-Id": os.environ.get("_X_AMZN_TRACE_ID")
        }
    )

def extract_links_from_sqs_record(record: dict) -> List[Link]:
    """
    Extract trace context links from a single SQS record.

    Checks messageAttributes first, then AWSTraceHeader.

    Args:
        record: SQS record dictionary

    Returns:
        List of valid span links extracted from the record
    """
    links = []
    xray_propagator = AwsXRayPropagator()

    if message_attributes := record.get("messageAttributes"):
        try:
            # Extract context using the w3c library's getter
            carrier_ctx = propagate.extract(message_attributes, getter=boto3sqs_getter)
            span_context = trace.get_current_span(carrier_ctx).get_span_context()

            # Only add valid span contexts as links
            if span_context.is_valid:
                links.append(Link(context=span_context))
                logger.debug(f"Added link from message attributes for message {record.get('messageId')}")
                return links  # Return early if we found a valid w3c context
        except Exception as e:
            logger.warning(f"Failed to extract trace context from message attributes: {e}")

    # Then check system attributes (AWS X-Ray propagation)
    if trace_header := record.get("attributes", {}).get("AWSTraceHeader"):
        try:
            # Create carrier with the X-Ray trace header
            carrier = {"X-Amzn-Trace-Id": trace_header}

            # Extract context using the X-Ray propagator
            carrier_ctx = xray_propagator.extract(carrier)
            span_context = trace.get_current_span(carrier_ctx).get_span_context()

            # Only add valid span contexts as links
            if span_context.is_valid:
                links.append(Link(context=span_context))
                logger.info(f"Added link from system attributes (AWSTraceHeader) for message {record.get('messageId')}")
        except Exception as e:
            logger.warning(f"Failed to extract trace context from AWSTraceHeader: {e}")

    return links

@contextmanager
def start_sqs_message_span(tracer: Tracer, record: dict) -> Iterator[Span]:
    """
    Starts and manages a span for processing a single SQS message.

    Extracts links, creates a CONSUMER span, sets standard messaging
    and quote attributes, and ensures the span is ended.

    Args:
        tracer: The OpenTelemetry Tracer instance.
        record: The SQS record dictionary.

    Yields:
        The created OpenTelemetry Span.
    """
    message_id = record.get("messageId")
    links = extract_links_from_sqs_record(record)
    # derive queue name for messaging.destination attributes
    queue_arn = record.get("eventSourceARN")
    queue_name = queue_arn.split(":")[-1] if queue_arn else None

    span = tracer.start_span(
        f"{queue_name} process",
        kind=SpanKind.CONSUMER,
        links=links,
        attributes={
            "faas.trigger": "pubsub",
            "messaging.message.id": message_id,
            "messaging.system": "aws_sqs",
            "messaging.operation.name": "process",
            "messaging.operation.type": "process",
            "messaging.destination.name": queue_name,
            "messaging.destination.kind": "queue"
        }
    )
    # We manually create the span, activate it using trace.use_span
    # only during the yield, and manually end it in `finally`.
    # This is because this function is a context manager itself. 
    # Using `tracer.start_as_current_span` would end the span 
    # immediately after the `yield`, which is too early.
    try:
        with trace.use_span(span, end_on_exit=False):
            yield span
    finally:
        span.end()

