---
layout: default
title: Concepts
nav_order: 3
has_children: true
---

# Core Concepts

Understanding the core concepts behind the Lambda OTLP Forwarder will help you make the most of its capabilities.

## Key Topics

- [Architecture](architecture): Learn about the components and how they work together
- [Processors](processors): Understand the different processor types and their use cases
- [Data Flow](data-flow): Deep dive into how telemetry data moves through the system

## Why This Approach?

The Lambda OTLP Forwarder was created to address the challenges of efficiently sending telemetry data from serverless applications to OTLP collectors without adding to cold start times. Traditional approaches using the OTEL/ADOT Lambda Layer extension deploy a sidecar agent, which:

- Increases resource usage
- Slows cold starts
- Drives up costs
- Requires VPC connectivity for collector access

Our solution provides a streamlined approach that:

- Maintains full telemetry capabilities
- Keeps resource consumption minimal
- Uses existing AWS infrastructure
- Provides flexible deployment options 