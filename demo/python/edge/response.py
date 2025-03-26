"""
Origin Response Lambda@Edge Function

This function serves as a passthrough for CloudFront origin responses, while
propagating the trace context from the origin response. It can be extended
for response transformation, header manipulation, or error handling.

For more information on Lambda@Edge, see:
https://docs.aws.amazon.com/lambda/latest/dg/lambda-edge.html
"""
from lambda_otel_lite import init_telemetry, create_traced_handler, SpanAttributes, TriggerType
from opentelemetry import trace, propagate
from opentelemetry.trace import SpanKind
from opentelemetry.sdk.extension.aws.trace import AwsXRayIdGenerator
from opentelemetry.propagators.aws import AwsXRayPropagator

# Configure X-Ray propagator for proper header format
propagate.set_global_textmap(AwsXRayPropagator())

# Initialize telemetry with X-Ray ID generator
tracer, completion_handler = init_telemetry(id_generator=AwsXRayIdGenerator())

# Create a CloudFront origin response event extractor
def cloudfront_origin_response_extractor(event, context):
    """Extract span attributes from CloudFront origin-response events"""
    attributes = {}
    carrier = None
    
    # Add Lambda context attributes
    if hasattr(context, "aws_request_id"):
        attributes["faas.invocation_id"] = context.aws_request_id
    
    if hasattr(context, "function_name"):
        attributes["faas.name"] = context.function_name
    
    # Extract CloudFront-specific attributes
    cf_record = event.get("Records", [{}])[0].get("cf", {})
    response = cf_record.get("response", {})
    request = cf_record.get("request", {})
    config = cf_record.get("config", {})
    
    # Extract trace context from response headers if present
    # This will allow the tracer to create a proper parent-child relationship
    if 'headers' in response and 'x-amzn-trace-id' in response['headers']:
        trace_header = response['headers']['x-amzn-trace-id'][0]['value']
        carrier = {'X-Amzn-Trace-Id': trace_header}
        # Mark that we found a trace context in the attributes
        attributes["origin.trace_context_found"] = "true"
    
    if config:
        attributes["cloudfront.distribution_id"] = config.get("distributionId", "")
        attributes["cloudfront.distribution_domain_name"] = config.get("distributionDomainName", "")
        attributes["cloudfront.request_id"] = config.get("requestId", "")
        attributes["cloudfront.event_type"] = config.get("eventType", "")
    
    if request:
        attributes["http.method"] = request.get("method", "")
        attributes["http.url"] = request.get("uri", "")
        attributes["http.client_ip"] = request.get("clientIp", "")
        
        # Extract key request headers
        request_headers = request.get("headers", {})
        if "user-agent" in request_headers:
            attributes["http.user_agent"] = request_headers["user-agent"][0].get("value", "")
        if "cache-control" in request_headers:
            attributes["http.request.cache_control"] = request_headers["cache-control"][0].get("value", "")
        
    if response:
        attributes["http.status_code"] = response.get("status", "")
        attributes["http.status_description"] = response.get("statusDescription", "")
        
        # Extract key response headers
        response_headers = response.get("headers", {})
        if "content-type" in response_headers:
            attributes["http.response.content_type"] = response_headers["content-type"][0].get("value", "")
        if "content-length" in response_headers:
            attributes["http.response.content_length"] = response_headers["content-length"][0].get("value", "")
    
    # Return span attributes with the carrier for proper parent-child relationship
    return SpanAttributes(
        trigger=TriggerType.HTTP,
        attributes=attributes,
        span_name="cloudfront-edge-response",
        kind=SpanKind.SERVER,
        carrier=carrier
    )

# Create the traced handler
traced_handler = create_traced_handler(
    "cloudfront-edge-response",
    completion_handler,
    attributes_extractor=cloudfront_origin_response_extractor
)

# Lambda handler
@traced_handler
def handler(event, lambda_context):
    """
    Lambda@Edge origin-response handler
    
    This function executes after CloudFront receives a response from the origin but before
    it forwards it to the viewer. This allows for modifying responses or implementing
    custom error handling.
    
    Parameters:
    -----------
    event : dict
        CloudFront event object
    lambda_context : LambdaContext
        Lambda context object
        
    Returns:
    --------
    dict
        The response to be forwarded to the viewer
    """
    # Get the current span created by the traced_handler decorator
    current_span = trace.get_current_span()
    
    # Extract the response from the CloudFront event
    cf_record = event['Records'][0]['cf']
    response = cf_record['response']
    request = cf_record['request']
    
    # Add events to the current span
    current_span.add_event("Processing response", {
        "method": request['method'],
        "uri": request['uri'],
        "status": response['status'],
        "origin": request.get('origin', {}).get('custom', {}).get('domainName', 'unknown')
    })
    
    # Example: Add a custom header to the response
    if 'headers' not in response:
        response['headers'] = {}
    
    response['headers']['x-otlp-processed'] = [{
        'key': 'X-OTLP-Processed',
        'value': 'true'
    }]
    
    current_span.add_event("Forwarding response", {
        "status": response['status'],
        "headers.count": str(len(response['headers']))
    })
    
    # Return the processed response
    return response 