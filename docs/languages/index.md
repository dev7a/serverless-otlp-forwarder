---
layout: default
title: Language Support
nav_order: 4
has_children: true
---

# Language Support

The Lambda OTLP Forwarder supports multiple programming languages through language-specific libraries that integrate with the OpenTelemetry SDKs.

## Supported Languages

### [Rust](rust)
- [otlp-stdout-client](https://crates.io/crates/otlp-stdout-client)
- Full async support with Tokio
- Comprehensive error handling
- Efficient binary serialization

### [Python](python)
- [otlp-stdout-adapter](https://pypi.org/project/otlp-stdout-adapter/)
- Easy integration with existing OTEL SDK
- Support for async and sync code
- Automatic context propagation

### [Node.js](nodejs)
- [@dev7a/otlp-stdout-exporter](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter)
- Compatible with OpenTelemetry JS
- Promise-based API
- TypeScript support

## Common Features

All language implementations provide:
- OTLP format support (protobuf and JSON)
- Compression options (gzip)
- Automatic AWS Lambda context integration
- Configurable through environment variables
- Batch processing capabilities

## Coming Soon

We're working on support for:
- Java
- .NET
- Go

Want to contribute? Check out our [Contributing Guide](../contributing) to help add support for more languages! 