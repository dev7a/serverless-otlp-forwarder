---
layout: home
title: Home
nav_order: 1
permalink: /
---

# Serverless OTLP Forwarder
{: .fs-9 }

A serverless solution for forwarding OpenTelemetry data from AWS Lambda functions to collectors with minimal overhead.
{: .fs-6 .fw-300 }

[Get Started](getting-started){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[View on GitHub](https://github.com/dev7a/serverless-otlp-forwarder){: .btn .fs-5 .mb-4 .mb-md-0 }

---

{: .warning }
> This project is under active development. APIs and features may change.

![Architecture Diagram](https://github.com/user-attachments/assets/961999d9-bb69-4ba7-92a2-9efef3909b74)
{: .text-center }

## Documentation
{: .text-delta }

- [Getting Started Guide](getting-started) - Installation and deployment
- [Architecture Overview](concepts/architecture) - Technical design and components
- [Configuration Guide](getting-started/configuration) - Configuration options
- [Language Support](languages) - Supported programming languages

## Key Features

- **Minimal Performance Impact**  
  Optimized for Lambda execution and cold start times

- **Secure by Design**  
  Leverages CloudWatch Logs for data transport, eliminating the need for direct collector exposure

- **Language Support**  
  Implementation available for Rust, Python, and Node.js

- **AWS Application Signals**  
  Experimental integration with AWS Application Signals

## Overview

The Serverless OTLP Forwarder implements an alternative approach to collecting OpenTelemetry data from AWS Lambda functions. Instead of using extension layers or sidecars, it utilizes CloudWatch Logs as a transport mechanism, reducing operational complexity and performance overhead.

The implementation consists of three main components:

1. Language-specific libraries that efficiently write telemetry data to standard output
2. CloudWatch Logs subscription that captures telemetry data
3. Forwarder function that processes and sends data to OTLP collectors

## Technical Considerations

### Benefits

- Reduced cold start impact compared to extension-based solutions
- No requirement for VPC connectivity or public collector endpoints
- Simplified deployment and maintenance
- Compatible with existing OpenTelemetry instrumentation

### Trade-offs

- CloudWatch Logs ingestion costs for telemetry data
  - Can be optimized using compression and protocol buffers
- Additional compute costs for the forwarder function
- Manual instrumentation required (no automatic instrumentation support)

## Background

This project addresses specific challenges in serverless observability, particularly the performance impact of traditional OpenTelemetry collection methods. The standard approach using OTEL/ADOT Lambda Layer extensions introduces significant overhead through sidecar agents, affecting both cold start times and runtime performance.

This becomes especially relevant in scenarios requiring memory-optimized Lambda functions, where the resource overhead of traditional collectors can offset the benefits of memory optimization. The forwarder approach provides an alternative that maintains telemetry capabilities while minimizing resource utilization. Read more about the underlying [concepts](concepts) and [architecture](concepts/architecture) of the project.

## Next Steps
{: .text-delta }

Refer to the [Getting Started Guide](getting-started) for installation and deployment instructions.
