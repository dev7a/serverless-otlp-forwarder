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


def _normalize_headers(headers: dict[str, str] | None) -> dict[str, str]:
    """Normalize header names to lowercase.

    HTTP header names are case-insensitive, so we normalize them to lowercase
    for consistent processing.

    Args:
        headers: Dictionary of headers or None

    Returns:
        Dictionary with lowercase header names. Empty dict if input is None.
    """
    if not headers:
        return {}
    return {k.lower(): v for k, v in headers.items()}


def default_extractor(event: Any, context: Any) -> SpanAttributes:
    """Default extractor for unknown event types.

    Args:
        event: Lambda event object (any type)
        context: Lambda context object

    Returns:
        SpanAttributes with basic Lambda invocation information
    """
    attributes: dict[str, Any] = {}

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


def api_gateway_v1_extractor(event: dict[str, Any], context: Any) -> SpanAttributes:
    """Extract span attributes from API Gateway v1 (REST API) events.

    Args:
        event: API Gateway v1 event
        context: Lambda context object

    Returns:
        SpanAttributes with HTTP and API Gateway specific attributes
    """
    # Start with default attributes
    base = default_extractor(event, context)
    attributes = base.attributes.copy()

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

    # API Gateway is always HTTPS
    attributes["url.scheme"] = "https"

    # Add protocol version from requestContext
    if "requestContext" in event:
        if protocol := event["requestContext"].get("protocol", "").lower():
            if protocol.startswith("http/"):
                attributes["network.protocol.version"] = protocol.replace("http/", "")

    # Add route with fallback to '/'
    attributes["http.route"] = event.get("resource", "/")

    # Add client IP from identity
    if "requestContext" in event and "identity" in event["requestContext"]:
        if source_ip := event["requestContext"]["identity"].get("sourceIp"):
            attributes["client.address"] = source_ip
        if user_agent := event["requestContext"]["identity"].get("userAgent"):
            attributes["user_agent.original"] = user_agent

    # Add server address from Host header
    if headers := _normalize_headers(event.get("headers")):
        if host := headers.get("host"):
            attributes["server.address"] = host

    # Get method and route for span name
    method = attributes.get("http.request.method", "HTTP")
    route = attributes["http.route"]

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
    # Start with default attributes
    base = default_extractor(event, context)
    attributes = base.attributes.copy()

    # Add HTTP method
    if "requestContext" in event and "http" in event["requestContext"]:
        if method := event["requestContext"]["http"].get("method"):
            attributes["http.request.method"] = method

    # Add path
    if raw_path := event.get("rawPath"):
        attributes["url.path"] = raw_path

    # Add query string if present and not empty
    if raw_query_string := event.get("rawQueryString"):
        if raw_query_string != "":
            attributes["url.query"] = raw_query_string

    # Always HTTPS for API Gateway
    attributes["url.scheme"] = "https"

    # Add protocol version
    if "requestContext" in event and "http" in event["requestContext"]:
        if protocol := event["requestContext"]["http"].get("protocol", "").lower():
            if protocol.startswith("http/"):
                attributes["network.protocol.version"] = protocol.replace("http/", "")

    # Add route with special handling for $default and fallback
    if route_key := event.get("routeKey"):
        if route_key == "$default":
            attributes["http.route"] = event.get("rawPath", "/")
        else:
            attributes["http.route"] = route_key
    else:
        attributes["http.route"] = "/"

    # Add client IP
    if "requestContext" in event and "http" in event["requestContext"]:
        http = event["requestContext"]["http"]
        if source_ip := http.get("sourceIp"):
            attributes["client.address"] = source_ip
        if user_agent := http.get("userAgent"):
            attributes["user_agent.original"] = user_agent

    if "requestContext" in event and "domainName" in event["requestContext"]:
        if domain_name := event["requestContext"]["domainName"]:
            attributes["server.address"] = domain_name

    # Get method and route for span name
    method = attributes.get("http.request.method", "HTTP")
    route = attributes["http.route"]

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
    # Start with default attributes
    base = default_extractor(event, context)
    attributes = base.attributes.copy()

    # Add HTTP method
    if method := event.get("httpMethod"):
        attributes["http.request.method"] = method

    # Add path and route with fallback to '/'
    path = event.get("path", "/")
    attributes["url.path"] = path
    attributes["http.route"] = path

    # Handle query string parameters if present and not empty
    if query_params := event.get("queryStringParameters"):
        if query_params:  # Check if dict is not empty
            query_parts = []
            for key, value in query_params.items():
                query_parts.append(f"{parse.quote(key)}={parse.quote(str(value))}")
            if query_parts:
                attributes["url.query"] = "&".join(query_parts)

    # Add ALB specific attributes
    if "requestContext" in event and "elb" in event["requestContext"]:
        if target_group_arn := event["requestContext"]["elb"].get("targetGroupArn"):
            attributes["alb.target_group_arn"] = target_group_arn

    # Extract attributes from headers
    headers = _normalize_headers(event.get("headers", {}))
    # Set URL scheme based on x-forwarded-proto
    if proto := headers.get("x-forwarded-proto"):
        attributes["url.scheme"] = proto.lower()
    else:
        attributes["url.scheme"] = "http"

    # Extract user agent
    if user_agent := headers.get("user-agent"):
        attributes["user_agent.original"] = user_agent

    # Extract server address from host
    if host := headers.get("host"):
        attributes["server.address"] = host

    # Extract client IP from x-forwarded-for
    if x_forwarded_for := headers.get("x-forwarded-for"):
        if client_ip := x_forwarded_for.split(",")[0].strip():
            attributes["client.address"] = client_ip

    # ALB uses HTTP/1.1
    attributes["network.protocol.version"] = "1.1"

    # Get method and route for span name
    method = attributes.get("http.request.method", "HTTP")
    route = attributes["http.route"]

    return SpanAttributes(
        trigger=TriggerType.HTTP,
        attributes=attributes,
        span_name=f"{method} {route}",
        carrier=event.get("headers", {}),
        kind=SpanKind.SERVER,
    )
