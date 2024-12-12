---
layout: default
title: Advanced Features
nav_order: 6
has_children: true
---

# Advanced Features

The Lambda OTLP Forwarder includes several advanced features for customization, optimization, and integration with various observability platforms.

## Processor Types

### [OTLP Stdout Processor](processors/otlp-stdout)
The standard processor that handles OTLP data from CloudWatch Logs:
- Supports both JSON and protobuf formats
- Handles compression
- Configurable batching and buffering
- Multiple collector support

### [AWS Application Signals](processors/app-signals)
Experimental processor for AWS Application Signals:
- Native integration with CloudWatch
- Optimized for AWS X-Ray
- Simplified configuration
- Automatic context propagation

## [Collector Configuration](collectors)

Configure multiple collectors with different settings:
- Authentication methods
- Endpoint configuration
- Headers and metadata
- Retry policies

## [Performance Optimization](performance)

Optimize your deployment for:
- Cold start times
- Memory usage
- Cost efficiency
- Throughput

## [Security](security)

Advanced security features:
- IAM role configuration
- VPC connectivity
- Encryption options
- Access controls

## [Monitoring](monitoring)

Monitor your forwarder:
- CloudWatch metrics
- Custom dashboards
- Alerting
- Cost tracking 