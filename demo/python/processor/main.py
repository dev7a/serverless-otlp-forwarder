import os
import json
from requests import Session
from lambda_otel_lite import init_telemetry, create_traced_handler
from opentelemetry.instrumentation.requests import RequestsInstrumentor
from opentelemetry.instrumentation.boto3sqs import Boto3SQSInstrumentor, boto3sqs_getter
from opentelemetry.trace import get_current_span, SpanKind, Link
from opentelemetry import propagate, trace
from opentelemetry.propagators.aws import AwsXRayPropagator
import logging
from typing import Optional, List, Mapping, Any

logger = logging.getLogger(__name__)

# Initialize telemetry
tracer, completion_handler = init_telemetry()

# Instrument requests library
RequestsInstrumentor().instrument()

# Instrument boto3sqs library
Boto3SQSInstrumentor().instrument()

http_session = Session()
target_url = os.environ.get("TARGET_URL")


def extract_links_from_sqs_record(record: dict) -> List[Link]:
    """
    Extract trace context links from a single SQS record.
    
    Args:
        record: SQS record dictionary
        
    Returns:
        List of valid span links extracted from the record
    """
    links = []
    xray_propagator = AwsXRayPropagator()
    
    # First check user-provided message attributes 
    message_attributes = record.get("messageAttributes", {})
    if message_attributes:
        try:
            # Extract context using the library's getter
            carrier_ctx = propagate.extract(message_attributes, getter=boto3sqs_getter)
            span_context = trace.get_current_span(carrier_ctx).get_span_context()
            
            # Only add valid span contexts as links
            if span_context.is_valid:
                links.append(Link(context=span_context))
                logger.info(f"Added link from message attributes for message {record.get('messageId')}")
                return links  # Return early if we found a valid context
        except Exception as e:
            logger.warning(f"Failed to extract trace context from message attributes: {e}")
    
    # Then check system attributes as per spec (AWS-provided)
    system_attributes = record.get("attributes", {})
    trace_header = system_attributes.get("AWSTraceHeader")
    
    if trace_header:
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


def extract_links_from_sqs_records(records: List[dict]) -> List[Link]:
    """
    Extract trace context links from all SQS records.
    
    Args:
        records: List of SQS record dictionaries
        
    Returns:
        List of valid span links extracted from all records
    """
    all_links = []
    
    for record in records:
        links = extract_links_from_sqs_record(record)
        all_links.extend(links)
            
    return all_links


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
    
    return SpanAttributes(
        trigger=TriggerType.PUBSUB,
        attributes=attributes,
        span_name="process-sqs-messages",
        kind=SpanKind.CONSUMER,
        links=links,
        carrier={
            "X-Amzn-Trace-Id": os.environ.get("_X_AMZN_TRACE_ID")
        }

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

        # Extract links for this specific message
        message_links = extract_links_from_sqs_record(record)

        with tracer.start_as_current_span(
            f"process-quote-{message_id}", 
            kind=SpanKind.CONSUMER,
            links=message_links  # Add links directly to this span
        ) as record_span:
            # Set messaging-specific attributes
            record_span.set_attribute("messaging.message_id", message_id)
            record_span.set_attribute("messaging.system", "aws.sqs")
            record_span.set_attribute("messaging.operation", "process")
            
            # Set quote-specific attributes to help identify the message content
            if quote:
                record_span.set_attribute("quote.id", str(quote.get("id", "")))
                record_span.set_attribute("quote.author", quote.get("author", ""))
                # Set a short preview of the quote text (first 50 chars)
                quote_text = quote.get("quote", "")
                if quote_text:
                    preview = (quote_text[:47] + "...") if len(quote_text) > 50 else quote_text
                    record_span.set_attribute("quote.text.preview", preview)
            
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
