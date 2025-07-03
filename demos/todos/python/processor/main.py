import os
import json
from requests import Session
import boto3
from lambda_otel_lite import init_telemetry, create_traced_handler
from opentelemetry.instrumentation.requests import RequestsInstrumentor
from opentelemetry import trace
import logging
from typing import List, Any, Dict, Tuple
from datetime import datetime

# Import the context manager
from sqs_tracing import start_sqs_message_span, sqs_event_extractor
from span_event import span_event, Level

logger = logging.getLogger(__name__)
bedrock_runtime = boto3.client('bedrock-runtime', region_name='us-east-1')
MODEL_ID = 'amazon.nova-micro-v1:0'  

# Initialize telemetry
tracer, completion_handler = init_telemetry()

# Instrument requests library
RequestsInstrumentor().instrument()

http_session = Session()
target_url = os.environ.get("TARGET_URL")

# Categories for TODOs
CATEGORIES = ["work", "personal", "study", "health", "home", "general"]

@tracer.start_as_current_span("consumer/processor/analyze")
def analyze_todo_with_bedrock(todo_text: str) -> Dict[str, Any]:
    """
    Use Amazon Bedrock to analyze a todo item and determine its category and priority.
    
    Args:
        todo_text: The text content of the todo
        
    Returns:
        Dict containing 'category' and 'priority'
    """
    # Optimized prompt
    prompt_content = (
        f'Analyze the following todo item: "{todo_text}".\n'
        '1. Assign it to exactly one of these categories: "home", "study", "friends", "entertainment", "health", "hobby", or "work".\n'
        '2. Estimate its priority as a number from 5 (highest priority: critical/urgent/important) to 1 (lowest priority: not urgent/not important).\n'
        'Respond with ONLY a JSON object in this exact format (no extra text):\n'
        '{"category": "<category>", "priority": "<priority>"}'
    )

    # Prepare payload per Nova Micro requirements (detailed message-based format)
    payload = {
        "schemaVersion": "messages-v1",
        "messages": [
            {
                "role": "user",
                "content": [
                    {"text": prompt_content}
                ]
            }
        ],
        "inferenceConfig": {
            "max_new_tokens": 100,
            "temperature": 0.2,
            "top_p": 0.9,
            "top_k": 50
        }
    }

    try:
        # Call Bedrock model
        response = bedrock_runtime.invoke_model(
            modelId=MODEL_ID,
            body=json.dumps(payload),
            accept='application/json',
            contentType='application/json'
        )
        # Parse and extract model output
        response_body = json.loads(response['body'].read())
        # Add LLM usage attributes to the current span
        current_span = trace.get_current_span()
        current_span.set_attribute("llm.request.model", MODEL_ID)
        if usage_data := response_body.get("usage"):
            current_span.set_attributes({
                "llm.usage.input_tokens": usage_data.get("inputTokens"),
                "llm.usage.output_tokens": usage_data.get("outputTokens"),
                "llm.usage.total_tokens": usage_data.get("totalTokens")
            })

        # Extract model output using the correct path for messages-v1 schema
        model_output = response_body['output']['message']['content'][0]['text']

        # Try to parse the JSON from the model output
        result = json.loads(model_output)
        priority_value = result.get('priority')
        if isinstance(priority_value, str):
            try:
                result['priority'] = int(priority_value)
            except ValueError:
                result['priority'] = 3  # Default to medium priority if conversion fails
                
        return result
    except Exception as e:
        span_event(
            name="demo.processor.error-analyzing-todo",
            body=f"Error analyzing todo with Bedrock: {str(e)}. Returning defaults",
            level=Level.WARN,
        )
        return {
            "category": "unknown",
            "priority": 3
        }


@tracer.start_as_current_span("consumer/processor/save")
def save_todo(todo: dict) -> dict:
    """
    Save a TODO to the backend service.
    Enhances the TODO with additional metadata if not already present.
    """
    # Enhance the TODO with additional metadata if not already present
    if "category" not in todo or "priority" not in todo:
        if todo_text := todo.get("todo"):
            analysis = analyze_todo_with_bedrock(todo_text)
            todo.setdefault("category", analysis.get("category"))
            todo.setdefault("priority", analysis.get("priority"))
    
    from datetime import timezone
    todo.setdefault("created_at", datetime.now(timezone.utc).isoformat())
    
    span_event(
        name="demo.processor.saving-todo",
        body=f"Saving TODO {todo.get('id')} to backend service at {target_url}",
        level=Level.INFO,
        attrs={
            "demo.todo.id": todo.get("id"),
            "demo.todo.title": todo.get("todo", "")[:50],  # Truncate long titles
            "demo.todo.category": todo.get("category", "unknown"),
            "demo.todo.priority": todo.get("priority", 0),
        },
    )
    
    response = http_session.post(
        target_url,
        json=todo,
        headers={
            "content-type": "application/json",
        },
    )

    response.raise_for_status()

    span_event(
        name="demo.processor.saved-todo",
        body=f"Successfully saved TODO {todo.get('id')} to backend with status code {response.status_code}",
        level=Level.INFO,
        attrs={
            "http.status_code": str(response.status_code),
        },
    )

    return response.json()

# Create the traced handler (using SQS extractor)
traced_handler = create_traced_handler(
    "sqs-processor",
    completion_handler,
    attributes_extractor=sqs_event_extractor
)

# Lambda handler
@traced_handler
def lambda_handler(event, lambda_context):
    """
    Lambda handler that processes SQS events containing TODOs.
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
        name="demo.processor.started-processing-todos",
        body=f"Started processing {batch_size} SQS messages",
        level=Level.INFO,
        attrs={
            "demo.todos.batch_size": batch_size,
        },
    )

    results = []
    for index, message in enumerate(event.get("Records", [])):
        if message_body := message.get("body"):
            # Use a context manager to handle the span
            # It handles link extraction, span creation, attribute setting, and ending the span
            with start_sqs_message_span(tracer, message) as processing_span:
                todo = json.loads(message_body)
                todo_text = todo.get("todo", "")
                
                span_event(
                    name="demo.processor.processing-todo",
                    body=f"Processing TODO with id: {todo.get('id')} ({index+1} of {batch_size})",
                    level=Level.INFO,
                )
                
                # Set span attributes for the TODO
                processing_span.set_attribute("todo.id", str(todo.get("id", "")))
                processing_span.set_attribute("todo.userId", str(todo.get("userId", "")))
                processing_span.set_attribute("todo.completed", todo.get("completed", False))
                
                # Set a short preview of the TODO text (first 50 chars)
                if todo_text:
                    preview = (todo_text[:47] + "...") if len(todo_text) > 50 else todo_text
                    processing_span.set_attribute("todo.text.preview", preview)
                
                # Use Bedrock for analysis if needed
                if not todo.get("category") or not todo.get("priority"):
                    analysis = analyze_todo_with_bedrock(todo_text)
                    
                    # Set default values from Bedrock analysis if not present
                    todo.setdefault("category", analysis.get("category"))
                    todo.setdefault("priority", analysis.get("priority"))
                
                # Set span attributes (now we can do this unconditionally)
                processing_span.set_attribute("todo.category", todo.get("category"))
                processing_span.set_attribute("todo.priority", todo.get("priority"))

                # Save the TODO within the body span's context
                try:
                    result = save_todo(todo)
                    results.append(result)
                except Exception as e:
                    span_event(
                        name="demo.processor.error-saving-todo",
                        body=f"Error saving TODO {todo.get('id')}: {str(e)}",
                        level=Level.ERROR,
                    )
                    raise

    return {
        "statusCode": 200,
        "body": json.dumps(
            {"message": "TODO processing complete", "processed": len(results)}
        ),
    }
