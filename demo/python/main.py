import os
import json
from requests import Session
from lambda_otel_lite import init_telemetry, create_traced_handler
from opentelemetry.instrumentation.requests import RequestsInstrumentor
from opentelemetry.trace import Status, StatusCode, get_current_span, SpanKind, Link
from opentelemetry import context, propagate, trace
import logging
from opentelemetry.propagate import get_global_textmap
from opentelemetry.propagators.textmap import Getter
from typing import Optional, List, Mapping, Any

logger = logging.getLogger(__name__)

# Initialize telemetry
tracer, completion_handler = init_telemetry()

# Instrument requests library
RequestsInstrumentor().instrument()

http_session = Session()
target_url = os.environ.get("TARGET_URL")


class Boto3SQSGetter(Getter[dict]):
    """
    Custom getter for extracting trace context from SQS message attributes.
    Implements the OpenTelemetry Getter protocol for SQS message attributes format.

    The SQS message attributes have the format:
    {
        "key": {
            "stringValue": "value",
            "dataType": "String"
        }
    }
    """

    def get(self, carrier: dict, key: str) -> Optional[List[str]]:
        """Get value from SQS message attributes for a given key."""
        msg_attr = carrier.get(key)
        if not isinstance(msg_attr, Mapping):
            return None

        value = msg_attr.get("stringValue")
        return [value] if value is not None else None

    def keys(self, carrier: dict) -> List[str]:
        """Get all keys from the carrier."""
        return list(carrier.keys())


# Create a single instance of the getter
boto3sqs_getter = Boto3SQSGetter()


def extract_links_from_sqs_records(records: List[dict]) -> List[Link]:
    """
    Extract trace context links from SQS records.
    
    Args:
        records: List of SQS record dictionaries
        
    Returns:
        List of valid span links extracted from message attributes
    """
    links = []
    
    for record in records:
        message_attributes = record.get("messageAttributes", {})
        if not message_attributes:
            continue
            
        try:
            # Extract context using our custom getter
            carrier_ctx = propagate.extract(message_attributes, getter=boto3sqs_getter)
            span_context = trace.get_current_span(carrier_ctx).get_span_context()
            
            # Only add valid span contexts as links
            if span_context.is_valid:
                links.append(Link(context=span_context))
        except Exception as e:
            logger.warning(f"Failed to extract trace context from message attributes: {e}")
            
    return links


@tracer.start_as_current_span("save_quote")
def save_quote(quote: dict):
    """
    Save a quote to the backend service.
    """
    span = get_current_span()
    span.add_event(
        "Saving quote",
        {
            "log.severity": "info",
            "log.message": "Sending quote to backend service",
            "quote.text": quote.get("quote", ""),
            "quote.author": quote.get("author", ""),
        },
    )

    response = http_session.post(
        target_url,
        json=quote,
        headers={
            "content-type": "application/json",
        },
    )
    response.raise_for_status()

    span.add_event(
        "Quote saved",
        {
            "log.severity": "info",
            "log.message": "Successfully saved quote to backend",
            "quote.id": str(quote.get("id", "")),
            "http.status_code": str(response.status_code),
        },
    )

    return response.json()


# Create a custom SQS event extractor
def sqs_event_extractor(event, context):
    """Extract span attributes from SQS events"""
    from lambda_otel_lite import SpanAttributes, TriggerType
    
    attributes = {}
    links = []
    
    # Add Lambda context attributes
    if hasattr(context, "aws_request_id"):
        attributes["faas.invocation_id"] = context.aws_request_id
    
    if hasattr(context, "invoked_function_arn"):
        arn = context.invoked_function_arn
        attributes["cloud.resource_id"] = arn
        # Extract account ID from ARN
        arn_parts = arn.split(":")
        if len(arn_parts) >= 5:
            attributes["cloud.account.id"] = arn_parts[4]
    
    # Add SQS specific attributes
    records = event.get("Records", [])
    if records:
        attributes["messaging.system"] = "aws.sqs"
        attributes["messaging.operation"] = "process"
        attributes["messaging.message.batch.size"] = len(records)
        
        # Extract trace context links from all records
        links = extract_links_from_sqs_records(records)
        
        # Extract the queue name from the first record if available
        if records and "eventSourceARN" in records[0]:
            queue_arn = records[0]["eventSourceARN"]
            attributes["messaging.destination.name"] = queue_arn.split(":")[-1]
    
    # Return span attributes with extracted links
    return SpanAttributes(
        trigger=TriggerType.PUBSUB,
        attributes=attributes,
        span_name="process-sqs-messages",
        kind=SpanKind.CONSUMER,
        links=links
    )


# Create the traced handler
traced_handler = create_traced_handler(
    "sqs-processor",
    completion_handler,
    attributes_extractor=sqs_event_extractor
)


# Lambda handler
@traced_handler
def lambda_handler(event, lambda_context):
    """
    Lambda handler that processes SQS events containing quotes.
    Creates a parent span for the batch and individual child spans for each message,
    with each child span linked to its original trace context.

    Args:
        event: Lambda event containing SQS records
        lambda_context: Lambda context

    Returns:
        dict: Response with status code 200 and processing results
    """
    # Get the current span (already created by the traced_handler decorator)
    parent_span = get_current_span()
    
    parent_span.add_event(
        "Processing started",
        {
            "log.severity": "info",
            "log.message": "Started processing SQS messages",
            "batch.size": len(event.get("Records", [])),
        },
    )

    results = []
    for record in event.get("Records", []):
        message_body = record.get("body", "{}")
        quote = json.loads(message_body)
        message_id = record.get("messageId")

        with tracer.start_as_current_span(
            "process-quote", kind=SpanKind.CONSUMER
        ) as record_span:
            record_span.set_attribute("messaging.message_id", message_id)
            record_span.add_event(
                "Processing quote",
                {
                    "log.severity": "info",
                    "log.message": "Processing individual quote from batch",
                    "quote.text": quote.get("quote", ""),
                    "quote.author": quote.get("author", ""),
                },
            )

            # Save the quote
            result = save_quote(quote)
            results.append(result)

    parent_span.add_event(
        "Processing completed",
        {
            "log.severity": "info",
            "log.message": f"Successfully processed {len(results)} quotes",
        },
    )

    return {
        "statusCode": 200,
        "body": json.dumps(
            {"message": "Quote processing complete", "processed": len(results)}
        ),
    }
