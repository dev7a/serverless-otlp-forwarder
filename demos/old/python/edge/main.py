"""
Simple Lambda@Edge Passthrough Function

This function serves as a passthrough for CloudFront origin requests. It can be
extended later to implement custom logic such as request transformation, header 
manipulation, or origin selection.

For more information on Lambda@Edge, see:
https://docs.aws.amazon.com/lambda/latest/dg/lambda-edge.html
"""
from lambda_otel_lite import init_telemetry, create_traced_handler, SpanAttributes, TriggerType, ProcessorMode
from opentelemetry import trace, propagate
from opentelemetry.trace import SpanKind
from opentelemetry.sdk.extension.aws.trace import AwsXRayIdGenerator
from opentelemetry.propagators.aws import AwsXRayPropagator

# Configure X-Ray propagator for proper header format
propagate.set_global_textmap(AwsXRayPropagator())

# Initialize telemetry with X-Ray ID generator
tracer, completion_handler = init_telemetry(
    id_generator=AwsXRayIdGenerator(), 
    processor_mode=ProcessorMode.ASYNC
)

# Create a CloudFront event extractor
def cloudfront_origin_request_extractor(event, context):
    """Extract span attributes from CloudFront origin-request events"""
    attributes = {}
    
    # Add Lambda context attributes
    if hasattr(context, "aws_request_id"):
        attributes["faas.invocation_id"] = context.aws_request_id
    
    if hasattr(context, "function_name"):
        attributes["faas.name"] = context.function_name
    
    # Extract CloudFront-specific attributes
    cf_record = event.get("Records", [{}])[0].get("cf", {})
    request = cf_record.get("request", {})
    config = cf_record.get("config", {})
    
    if config:
        attributes["cloudfront.distribution_id"] = config.get("distributionId", "")
        attributes["cloudfront.distribution_domain_name"] = config.get("distributionDomainName", "")
        attributes["cloudfront.request_id"] = config.get("requestId", "")
        attributes["cloudfront.event_type"] = config.get("eventType", "")
    
    if request:
        attributes["http.method"] = request.get("method", "")
        attributes["http.url"] = request.get("uri", "")
        
        # Extract headers with proper handling of CloudFront header structure
        headers = request.get("headers", {})
        if "user-agent" in headers and headers["user-agent"]:
            attributes["http.user_agent"] = headers["user-agent"][0].get("value", "")
            
        # Extract origin information
        origin = request.get("origin", {}).get("custom", {})
        if origin:
            attributes["origin.domain_name"] = origin.get("domainName", "")
            attributes["origin.protocol"] = origin.get("protocol", "")
            
        # Extract client IP
        if "clientIp" in request:
            attributes["http.client_ip"] = request.get("clientIp")
    
    # Return span attributes
    return SpanAttributes(
        trigger=TriggerType.HTTP,
        attributes=attributes,
        span_name="cloudfront-edge-request",
        kind=SpanKind.SERVER
    )

# Create the traced handler
traced_handler = create_traced_handler(
    "cloudfront-edge",
    completion_handler,
    attributes_extractor=cloudfront_origin_request_extractor
)

# Lambda handler
@traced_handler
def handler(event, context):
    """
    Lambda@Edge origin-request handler
    
    This function executes after CloudFront has checked its cache but before
    it forwards the request to the origin. This allows for modifying requests
    going to the origin without affecting caching behavior.
    
    Parameters:
    -----------
    event : dict
        CloudFront event object
    context : LambdaContext
        Lambda context object
        
    Returns:
    --------
    dict
        The request to be forwarded to the origin
    """
    # Get the current span created by the traced_handler decorator
    current_span = trace.get_current_span()
    # Extract the request from the CloudFront event
    request = event['Records'][0]['cf']['request']
    
    # Add events to the current span
    current_span.add_event(
        name="edge.request",
        attributes={
            "event.severity_text": "INFO",
            "event.severity_number": 9,
            "event.body": f"received request from ip: {request['clientIp']} for {request['uri']}"
        }
    )
    
    # Initialize headers if not present
    if 'headers' not in request:
        request['headers'] = {}
        
    # Inject trace context into headers to be sent to the origin via CloudFront
    carrier = {}
    propagate.inject(carrier)
    
    # Convert to CloudFront header format and add to request
    for key, value in carrier.items():
        header_key = key.lower()  # CloudFront expects lowercase header names
        request['headers'][header_key] = [{
            'key': header_key,
            'value': value
        }]
    
    current_span.add_event(
        name="edge.forwarding",
        attributes={
            "tracecontext.injected": "true",
            "headers.count": str(len(request['headers']))
        }
    )

    # Return the modified request with trace context headers
    return request 