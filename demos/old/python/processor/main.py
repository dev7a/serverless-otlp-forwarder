import os
import json
from requests import Session
from lambda_otel_lite import init_telemetry, create_traced_handler
from opentelemetry.instrumentation.requests import RequestsInstrumentor
from opentelemetry import trace
import logging
from typing import List, Any

# Import the new context manager
from sqs_tracing import start_sqs_message_span, sqs_event_extractor
from span_event import span_event, Level
logger = logging.getLogger(__name__)

# Initialize telemetry
tracer, completion_handler = init_telemetry()

# Instrument requests library
RequestsInstrumentor().instrument()


http_session = Session()
target_url = os.environ.get("TARGET_URL")


@tracer.start_as_current_span("processor.save-quote")
def save_quote(quote: dict):
    """
    Save a quote to the backend service.
    """
    span_event(
        name="demo.processor.saving-quote",
        body=f"Saving quote {quote.get('id')} to backend service at {target_url}",
        level=Level.INFO,
        attrs={
            "demo.quote.id": quote.get("id"),
            "demo.quote.text": quote.get("quote"),
            "demo.quote.author": quote.get("author"),
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

    span_event(
        name="demo.processor.saved-quote",
        body=f"Successfully saved quote {quote.get('id')} to backend with status code {response.status_code}",
        level=Level.INFO,
        attrs={
            "http.status_code": str(response.status_code),
        },
    )

    return response.json()


# Create the traced handler (using default extractor)
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
    using the sqs_tracing module for per-message span management.

    Args:
        event: Lambda event containing SQS records
        lambda_context: Lambda context

    Returns:
        dict: Response with status code 200 and processing results
    """
    batch_size = len(event.get("Records", []))
    span_event(
        name="demo.processor.started-processing-quotes",
        body=f"Started processing {batch_size} SQS messages",
        level=Level.INFO,
        attrs={
            "demo.quotes.batch_size": batch_size, # Use calculated batch_size
        },
    )

    results = []
    for index, message in enumerate(event.get("Records", [])):
        if message_body := message.get("body"):
            # Use a context manager to handle the span
            # It handles link extraction, span creation, attribute setting, and ending the span
            with start_sqs_message_span(tracer, message) as processing_span:
                quote = json.loads(message_body)
                span_event(
                    name="demo.processor.processing-quote",
                    body=f"Processing quote with id: {quote.get('id')} ({index+1} of {batch_size})",
                    level=Level.INFO,
                )
                processing_span.set_attribute("quote.id", str(quote.get("id", "")))
                processing_span.set_attribute("quote.author", quote.get("author", ""))
                # Set a short preview of the quote text (first 50 chars)
                quote_text = quote.get("quote", "")
                if quote_text:
                    preview = (quote_text[:47] + "...") if len(quote_text) > 50 else quote_text
                    processing_span.set_attribute("quote.text.preview", preview)

                # Save the quote within the body span's context
                try:
                    result = save_quote(quote)
                    results.append(result)
                except Exception as e:
                    span_event(
                        name="demo.processor.error-saving-quote",
                        body=f"Error saving quote {quote.get('id')}: {str(e)}",
                        level=Level.ERROR,
                    )
                    raise

    return {
        "statusCode": 200,
        "body": json.dumps(
            {"message": "Quote processing complete", "processed": len(results)}
        ),
    }
