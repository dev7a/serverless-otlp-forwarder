# SigV4 Authentication Example

This example demonstrates how to use the `otlp-sigv4-client` to send OpenTelemetry traces to AWS X-Ray using SigV4 authentication.

## Prerequisites

The example uses AWS's standard credential provider chain, so you need to have valid AWS credentials configured in your environment.

## Running the Example

You need to set the AWS profile to use for the example, and have permissions to send traces to AWS X-Ray OTLP endpoint.

```bash
AWS_PROFILE=your_profile cargo run --example sigv4_auth
```

This will:
1. Load AWS credentials using the provider chain
2. Create a SigV4-authenticated HTTP client
3. Configure an OTLP exporter to use AWS X-Ray OTLP endpoint
4. Create and send a sample trace with attributes

## Example Output

Traces will be available in the AWS Transaction Search CloudWatch console.

![Example Output](https://github.com/user-attachments/assets/fbe39c1e-ce3e-47e8-9f46-748937c64615)

Spans are also recorded in the `aws/spans` log group in this format:
```json
{
    "resource": {
        "attributes": {
            "service.name": "example-service",
            "service.version": "0.10.0"
        }
    },
    "scope": {
        "name": "example",
        "version": ""
    },
    "traceId": "48df1f88137fd27cdaaa2d688d942a6e",
    "spanId": "618c0744794b5d4e",
    "parentSpanId": "ca6d4c0a56024329",
    "flags": 1,
    "name": "child-2",
    "kind": "INTERNAL",
    "startTimeUnixNano": 1741056915877039000,
    "endTimeUnixNano": 1741056915877059000,
    "durationNano": 20000,
    "events": [
        {
            "timeUnixNano": 1741056915877057000,
            "name": "child event",
            "attributes": {
                "child.key": "child.value"
            }
        }
    ],
    "status": {
        "code": "UNSET"
    }
}
```
## Troubleshooting

1. If you see authentication errors:
   - Check that you have valid AWS credentials configured in one of the supported locations
   - Verify your credentials have the necessary X-Ray permissions

2. If traces don't appear:
   - The AWS X-Ray console may take a few minutes to display new traces
   - Verify your AWS region matches the endpoint configuration
   - Check for any error messages in the console output