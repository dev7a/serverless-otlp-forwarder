# SigV4 Authentication Example

This example demonstrates how to use the `otlp-sigv4-client` to send OpenTelemetry traces to AWS X-Ray using SigV4 authentication.

## Prerequisites

The example uses AWS's standard credential provider chain, so you need to have valid AWS credentials configured in your environment.

## Running the Example

```bash
cargo run --example sigv4_auth
```

This will:
1. Load AWS credentials using the provider chain
2. Create a SigV4-authenticated HTTP client
3. Configure an OTLP exporter to use AWS X-Ray
4. Create and send a sample trace with attributes

## Example Output

You should see the trace appear in your AWS X-Ray console after a few seconds. The trace will include:
- Service name: "example-service"
- Service version: (from Cargo.toml)
- A single span named "main" with an attribute "example.key" = "example.value"

## Troubleshooting

1. If you see authentication errors:
   - Check that you have valid AWS credentials configured in one of the supported locations
   - Verify your credentials have the necessary X-Ray permissions
   - Run `aws configure list` to see which credentials are being used

2. If traces don't appear:
   - The AWS X-Ray console may take a few minutes to display new traces
   - Verify your AWS region matches the endpoint configuration
   - Check for any error messages in the console output