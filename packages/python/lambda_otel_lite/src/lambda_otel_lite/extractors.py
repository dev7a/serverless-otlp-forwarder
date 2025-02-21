"""
Event extractors for lambda-otel-lite.

This module provides extractors for common AWS Lambda event types, converting them
into OpenTelemetry span attributes with proper context propagation.
"""

from typing import Any
from urllib import parse

from opentelemetry.trace import Link, SpanKind


class TriggerType:
    """Standard trigger types for Lambda invocations.

    These are the standard trigger types, but users can provide custom types
    by passing any string value.
    """

    DATASOURCE = "datasource"  # Database or storage operations
    HTTP = "http"  # HTTP/REST APIs and web requests
    PUBSUB = "pubsub"  # Message queues and event buses
    TIMER = "timer"  # Scheduled and time-based triggers
    OTHER = "other"  # Default for unknown triggers


class SpanAttributes:
    """Container for span attributes extracted from Lambda events.

    This class holds all the information needed to create a span from a Lambda event,
    including attributes, context propagation headers, and span configuration.
    """

    def __init__(
        self,
        trigger: str,
        attributes: dict[str, Any],
        span_name: str | None = None,
        carrier: dict[str, str] | None = None,
        kind: SpanKind = SpanKind.SERVER,
        links: list[Link] | None = None,
    ):
        """Initialize span attributes.

        Args:
            trigger: The type of trigger that caused this Lambda invocation.
                    Can be one of the standard TriggerType values or any custom string.
            attributes: Extracted attributes specific to this event type
            span_name: Optional custom span name. If not provided, a default will be used
            carrier: Optional carrier dictionary for context propagation
            kind: The span kind (default: SERVER)
            links: Optional span links (e.g., for batch processing)
        """
        self.trigger = trigger
        self.attributes = attributes
        self.span_name = span_name
        self.carrier = carrier
        self.kind = kind
        self.links = links


def default_extractor(event: Any, context: Any) -> SpanAttributes:
    """Default extractor for unknown event types.

    Args:
        event: Lambda event object (any type)
        context: Lambda context object

    Returns:
        SpanAttributes with basic Lambda invocation information
    """
    attributes = {}

    # Add invocation ID if available
    if hasattr(context, "aws_request_id"):
        attributes["faas.invocation_id"] = context.aws_request_id

    # Add function ARN and account ID if available
    if hasattr(context, "invoked_function_arn"):
        arn = context.invoked_function_arn
        attributes["cloud.resource_id"] = arn
        # Extract account ID from ARN (arn:aws:lambda:region:account-id:...)
        arn_parts = arn.split(":")
        if len(arn_parts) >= 5:
            attributes["cloud.account.id"] = arn_parts[4]

    return SpanAttributes(trigger=TriggerType.OTHER, attributes=attributes)


def _normalize_headers(headers: dict[str, str] | None) -> dict[str, str] | None:
    """Normalize header names to lowercase.

    HTTP header names are case-insensitive, so we normalize them to lowercase
    for consistent processing.
    """
    if not headers:
        return headers
    return {k.lower(): v for k, v in headers.items()}


def api_gateway_v1_extractor(event: dict[str, Any], context: Any) -> SpanAttributes:
    """Extract span attributes from API Gateway v1 (REST API) events.

    Args:
        event: API Gateway v1 event
        context: Lambda context object

    Returns:
        SpanAttributes with HTTP and API Gateway specific attributes
    """
    attributes = {}

    # Start with default attributes
    base = default_extractor(event, context)
    attributes.update(base.attributes)

    # Add HTTP method
    if method := event.get("httpMethod"):
        attributes["http.request.method"] = method

    # Add path
    if path := event.get("path"):
        attributes["url.path"] = path

    # Handle query string parameters
    if query_params := event.get("multiValueQueryStringParameters"):
        query_parts = []
        for key, values in query_params.items():
            if isinstance(values, list):
                for value in values:
                    query_parts.append(f"{parse.quote(key)}={parse.quote(value)}")
        if query_parts:
            attributes["url.query"] = "&".join(query_parts)

    # Always HTTPS for API Gateway
    attributes["url.scheme"] = "https"

    # Add protocol version
    if "requestContext" in event:
        if protocol := event["requestContext"].get("protocol", "").lower():
            if protocol.startswith("http/"):
                attributes["network.protocol.version"] = protocol.replace("http/", "")

    # Add route
    if route := event.get("resource"):
        attributes["http.route"] = route

    # Add client IP
    if "requestContext" in event and "identity" in event["requestContext"]:
        if source_ip := event["requestContext"]["identity"].get("sourceIp"):
            attributes["client.address"] = source_ip

    # Add user agent and server address from normalized headers
    if headers := _normalize_headers(event.get("headers")):
        if user_agent := headers.get("user-agent"):
            attributes["user_agent.original"] = user_agent

    # Use domain name for server address (like Node.js)
    if "requestContext" in event and "domainName" in event["requestContext"]:
        if domain_name := event["requestContext"]["domainName"]:
            attributes["server.address"] = domain_name

    # Get method and route for span name
    method = attributes.get("http.request.method", "HTTP")
    route = attributes.get("http.route", "/")

    return SpanAttributes(
        trigger=TriggerType.HTTP,
        attributes=attributes,
        span_name=f"{method} {route}",
        carrier=event.get("headers", {}),
        kind=SpanKind.SERVER,
    )


def api_gateway_v2_extractor(event: dict[str, Any], context: Any) -> SpanAttributes:
    """Extract span attributes from API Gateway v2 (HTTP API) events.

    Args:
        event: API Gateway v2 event
        context: Lambda context object

    Returns:
        SpanAttributes with HTTP and API Gateway specific attributes
    """
    attributes = {}

    # Start with default attributes
    base = default_extractor(event, context)
    attributes.update(base.attributes)

    # Add HTTP method
    if "requestContext" in event and "http" in event["requestContext"]:
        if method := event["requestContext"]["http"].get("method"):
            attributes["http.request.method"] = method

    # Add path
    if raw_path := event.get("rawPath"):
        attributes["url.path"] = raw_path

    # Add query string
    if raw_query_string := event.get("rawQueryString"):
        attributes["url.query"] = raw_query_string

    # Always HTTPS for API Gateway
    attributes["url.scheme"] = "https"

    # Add protocol version
    if "requestContext" in event and "http" in event["requestContext"]:
        if protocol := event["requestContext"]["http"].get("protocol", "").lower():
            if protocol.startswith("http/"):
                attributes["network.protocol.version"] = protocol.replace("http/", "")

    # Add route if available
    if route := event.get("routeKey"):
        if route == "$default":
            route = event.get("rawPath", "/")
        attributes["http.route"] = route

    # Add client IP
    if "requestContext" in event and "http" in event["requestContext"]:
        if source_ip := event["requestContext"]["http"].get("sourceIp"):
            attributes["client.address"] = source_ip

    # Add user agent and server address from normalized headers
    if headers := _normalize_headers(event.get("headers")):
        if user_agent := headers.get("user-agent"):
            attributes["user_agent.original"] = user_agent

    # Use domain name for server address (like Node.js)
    if "requestContext" in event and "domainName" in event["requestContext"]:
        if domain_name := event["requestContext"]["domainName"]:
            attributes["server.address"] = domain_name

    # Get method and route for span name
    method = attributes.get("http.request.method", "HTTP")
    route = attributes.get("http.route", "/")

    return SpanAttributes(
        trigger=TriggerType.HTTP,
        attributes=attributes,
        span_name=f"{method} {route}",
        carrier=event.get("headers", {}),
        kind=SpanKind.SERVER,
    )


def alb_extractor(event: dict[str, Any], context: Any) -> SpanAttributes:
    """Extract span attributes from Application Load Balancer events.

    Args:
        event: ALB event
        context: Lambda context object

    Returns:
        SpanAttributes with HTTP and ALB specific attributes
    """
    attributes = {}

    # Start with default attributes
    base = default_extractor(event, context)
    attributes.update(base.attributes)

    # Add HTTP method
    if method := event.get("httpMethod"):
        attributes["http.request.method"] = method

    # Add path and route
    if path := event.get("path"):
        attributes["url.path"] = path
        attributes["http.route"] = path  # For ALB, route is the same as path

    # Handle query string parameters
    if query_params := event.get("multiValueQueryStringParameters"):
        query_parts = []
        for key, values in query_params.items():
            if isinstance(values, list):
                for value in values:
                    query_parts.append(f"{parse.quote(key)}={parse.quote(value)}")
        if query_parts:
            attributes["url.query"] = "&".join(query_parts)

    # ALB can be HTTP or HTTPS, default to HTTP
    attributes["url.scheme"] = "http"
    # ALB uses HTTP/1.1
    attributes["network.protocol.version"] = "1.1"

    # Add ALB specific attributes
    if "requestContext" in event and "elb" in event["requestContext"]:
        if target_group_arn := event["requestContext"]["elb"].get("targetGroupArn"):
            attributes["alb.target_group_arn"] = target_group_arn

    # Add headers from normalized headers
    if headers := _normalize_headers(event.get("headers")):
        # Add client IP from X-Forwarded-For
        if forwarded_for := headers.get("x-forwarded-for"):
            if client_ip := forwarded_for.split(",")[0].strip():
                attributes["client.address"] = client_ip

        # Add user agent
        if user_agent := headers.get("user-agent"):
            attributes["user_agent.original"] = user_agent

        # Add host as server address
        if host := headers.get("host"):
            attributes["server.address"] = host

    # Get method and path for span name
    method = attributes.get("http.request.method", "HTTP")
    path = attributes.get("url.path", "/")

    return SpanAttributes(
        trigger=TriggerType.HTTP,
        attributes=attributes,
        span_name=f"{method} {path}",
        carrier=event.get("headers", {}),
        kind=SpanKind.SERVER,
    )
