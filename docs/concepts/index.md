---
layout: default
title: Concepts
nav_order: 3
has_children: true
---

# Core Concepts
{: .fs-9 }

Technical overview of Serverless OTLP Forwarder's architecture and components.
{: .fs-6 .fw-300 }

## Overview
{: .text-delta }

OpenTelemetry is a vendor-neutral, open-source framework for collecting, processing, and exporting telemetry data. In a serverless environment, the implementation of a telemetry pipeline is not as straightforward as in a traditional environment. Long running processes have the advantage of being able to maintain a persistent network connection to the collector, state in memory to buffer data and to periodically flush it to the collector, and retry logic to handle transient failures. And they can afford longer cold start times, needed to initialize the instrumentation libraries, because the initialization is done once and then the process is kept alive.

Conversely, Lambda functions are short-lived processes that start and stop frequently. They do not have the ability to guarantee a persistent network connection to the collector, and, while they can buffer data in memory, flushing it to the collector periodically is not trivial, because the execution environment is frozen after the function invocation ends. 

A solution to these challenges is to minimize the cold start impact by limiting the number of instrumentation libraries loaded during initialization, avoiding the establishment of network connections for sending telemetry data, and utilizing the lowest overhead I/O mechanism possible. By writing telemetry data to stdout in a structured format, Lambda functions can leverage the built-in CloudWatch Logs integration as a durable transport layer, without adding significant latency or complexity to the function execution.

The Serverless OTLP Forwarder aims to provide a solution to these challenges, at least on Lambda. It implements a serverless telemetry pipeline using AWS services and the OpenTelemetry Protocol (OTLP). The system consists of several key components:

1. **Transport Layer**: Uses CloudWatch Logs as a durable transport mechanism
2. **Processing Layer**: Lambda function with pluggable processors
3. **Protocol Layer**: OTLP-compliant data formatting and transmission
4. **Integration Layer**: Connections to observability backends


## System Architecture
{: .text-delta }

![image](https://github.com/user-attachments/assets/7af44a01-10d5-439c-89bb-27a75cf21c41)

## Technical Components
{: .text-delta }

### [Architecture](architecture)
{: .text-delta }

The system architecture is designed for:
- Durability through CloudWatch Logs
- Scalability via serverless components
- Reliability with automatic retries
- Security through AWS IAM and encryption

### [Processors](processors)
{: .text-delta }

Processors handle:
- Protocol transformation (JSON/Protobuf)
- Data buffering and batching
- Error handling and retries
- Collector authentication

## Implementation Details
{: .text-delta }

### Data Flow
{: .text-delta }

![Data Flow Diagram](https://github.com/user-attachments/assets/2252d2e4-d30d-4a1c-b433-9b552c1ad383)

<details markdown="1">
<summary>View sequence diagram source code</summary>

```
sequenceDiagram
    participant App as Lambda Function
    participant CW as CloudWatch Logs
    participant Fwd as Forwarder
    participant Proc as Processor
    participant Col as Collector

    App->>CW: Write OTLP data to stdout
    Note over App,CW: Structured JSON/protobuf
    CW->>Fwd: Forward via subscription
    Note over CW,Fwd: Filter pattern match
    Fwd->>Proc: Process log events
    Note over Fwd,Proc: Transform & batch
    Proc->>Col: Forward via OTLP/HTTP
    Note over Proc,Col: Compressed & authenticated
```

</details>

### AWS Integration
{: .text-delta }

The forwarder integrates with:
- **Lambda**: Function runtime and execution
- **CloudWatch**: Log aggregation and filtering
- **IAM**: Access control and permissions
- **Secrets Manager**: Collector credentials

### Performance Considerations
{: .text-delta }

Key performance factors:
- Cold start optimization (arm64 architecture)
- Efficient log processing and filtering
- Batching and compression strategies
- Memory and timeout configuration
- Concurrent execution limits

### Security Model
{: .text-delta }

Security implementation:
- IAM roles and policies with least privilege
- TLS encryption for data in transit to the collector
- Secrets Manager for credentials
- Network security with VPC support
- Audit logging capabilities with Cloudtrail

## Technical Documentation
{: .text-delta }

- [Architecture Details](architecture)
- [Processor Implementation](processors)
- [Deployment Configuration](../deployment)