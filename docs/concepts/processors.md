---
layout: default
title: Processors
parent: Concepts
nav_order: 2
---

# Processors
{: .fs-9 }

Understanding the available processors in Serverless OTLP Forwarder.
{: .fs-6 .fw-300 }

## Overview
{: .text-delta }

Processors are responsible for handling telemetry data from CloudWatch Logs and forwarding it to collectors. The Serverless OTLP Forwarder supports two types of processors, each designed for specific use cases.

## OTLP Stdout Processor
{: .text-delta }

The OTLP Stdout processor is the primary processor for handling OpenTelemetry data. It processes OTLP-formatted log entries from CloudWatch Logs and forwards them to OTLP collectors.

### Features
- Processes OTLP data written to stdout by instrumented functions
- Supports both JSON and protobuf formats (based on application configuration)
- Handles compressed data (if configured by the application)
- Forwards to any OTLP-compatible collector
- Supports multiple collectors in parallel

### Authentication
The processor supports two authentication methods:
1. **Custom Headers**:
   - Uses headers stored in AWS Secrets Manager
   - Compatible with OTEL_EXPORTER_OTLP_HEADERS format
   - Supports any collector-specific authentication scheme

2. **AWS SigV4**:
   - For sending data to AWS Application Signals OTLP endpoint
   - Uses the forwarder's IAM role
   - No additional credentials needed

{: .note }
For detailed configuration instructions, including how to set up multiple collectors, see the [Collector Configuration](../getting-started/configuration#collector-configuration) section.

## AWS Spans Processor (Experimental)
{: .text-delta }

{: .warning }
This processor is experimental and should not be used in production environments.

The AWS Spans processor provides compatibility between AWS Application Signals and standard OTLP collectors.

### Features
- Reads trace data from the `aws/span` log group
- Converts AWS Application Signals format into OTLP JSON
- Forwards converted data to standard OTLP collectors
- May have limitations and known issues

## Best Practices
{: .text-delta }

1. **Processor Selection**:
   - Use the OTLP Stdout processor for standard observability platforms
   - Consider the experimental status of the AWS Spans processor

2. **Authentication**:
   - Store collector credentials securely in Secrets Manager
   - Use SigV4 when sending data to AWS Application Signals OTLP endpoint
   - Rotate API keys regularly

3. **Performance**:
   - Configure appropriate OpenTelemetry SDK settings in your applications
   - Use the BatchSpanProcessor in your instrumented functions
   - Consider compression for large payload volumes

4. **Monitoring**:
   - Monitor Lambda execution metrics
   - Use the generated OpenTelemetry traces for operational insights
   - Set up appropriate alerting on Lambda errors