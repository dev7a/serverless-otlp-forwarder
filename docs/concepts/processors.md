---
layout: default
title: Processors
parent: Concepts
nav_order: 2
---

# Processors

The Lambda OTLP Forwarder supports different processors for handling telemetry data. Each processor is designed for specific use cases and can be configured independently.

## Available Processors

### OTLP Stdout Processor

The standard processor that handles OTLP data from CloudWatch Logs:

- **Features**
  - Supports both JSON and protobuf formats
  - Handles GZIP compression
  - Configurable batching and buffering
  - Multiple collector support
  - Custom authentication methods

- **Configuration**
  ```yaml
  ProcessorType: otlp-stdout
  RouteAllLogs: true
  ```

- **Use Cases**
  - General telemetry collection
  - Third-party observability platforms
  - Custom OTLP collectors

### AWS Application Signals Processor

Experimental processor specifically for AWS Application Signals:

- **Features**
  - Native CloudWatch integration
  - Optimized for AWS X-Ray
  - Simplified configuration
  - Automatic context propagation

- **Configuration**
  ```yaml
  ProcessorType: aws-appsignals
  ```

- **Use Cases**
  - AWS-native observability
  - X-Ray integration
  - CloudWatch Application Signals

## Record Format

### OTLP Stdout Format

Each log record includes:
```json
{
  "__otel_otlp_stdout": "package-name@version",
  "source": "service-name",
  "endpoint": "collector-endpoint",
  "method": "POST",
  "content-type": "application/json|application/x-protobuf",
  "payload": "base64-or-plain-content",
  "base64": true|false,
  "content-encoding": "gzip|none"
}
```

### Application Signals Format

Application Signals uses AWS X-Ray format with extensions for OTLP data.

## Processor Selection

Choose your processor based on your needs:

1. **OTLP Stdout Processor** if you:
   - Use third-party observability platforms
   - Need multiple collector support
   - Require custom configuration
   - Want maximum flexibility

2. **Application Signals Processor** if you:
   - Use AWS X-Ray
   - Want native CloudWatch integration
   - Prefer simplified setup
   - Don't need multiple collectors

> [!WARNING]
> The AWS Application Signals processor is experimental. Do not enable both processors simultaneously as this could lead to duplicate processing.

## Best Practices

1. **Data Format**
   - Use protobuf for better performance
   - Enable compression for large payloads
   - Consider payload size limits

2. **Processing**
   - Configure appropriate batch sizes
   - Set reasonable timeouts
   - Monitor processing errors

3. **Resource Usage**
   - Monitor Lambda memory usage
   - Watch CloudWatch Logs costs
   - Configure appropriate concurrency

4. **Error Handling**
   - Implement retry strategies
   - Set up error alerting
   - Monitor failed deliveries

## Configuration Examples

### OTLP Stdout with Multiple Collectors

```yaml
ProcessorType: otlp-stdout
RouteAllLogs: true
CollectorsSecretsKeyPrefix: lambda-otlp-forwarder/keys
```

### Application Signals Only

```yaml
ProcessorType: aws-appsignals
DeployDemo: false
```

### Development Setup

```yaml
ProcessorType: otlp-stdout
DeployDemo: true
DemoExporterProtocol: http/json
DemoExporterCompression: none
```

## Next Steps

- [Configure collectors](../deployment/collectors)
- [Performance optimization](../advanced/performance)
- [Monitoring setup](../advanced/monitoring) 