---
layout: default
title: Language Support
nav_order: 4
has_children: true
---

# Language Support
{: .fs-9 }

Choose your preferred programming language and get started with Lambda OTLP Forwarder.
{: .fs-6 .fw-300 }

## Supported Languages
{: .text-delta }

| Language | Package | Version | Status |
|:---------|:--------|:--------|:-------|
| [Rust](rust) | `otlp-stdout-client` | [![Crates.io](https://img.shields.io/crates/v/otlp-stdout-client.svg)](https://crates.io/crates/otlp-stdout-client) | Stable |
| [Python](python) | `otlp-stdout-adapter` | [![PyPI](https://img.shields.io/pypi/v/otlp-stdout-adapter.svg)](https://pypi.org/project/otlp-stdout-adapter/) | Stable |
| [Node.js](nodejs) | `@dev7a/otlp-stdout-exporter` | [![npm](https://img.shields.io/npm/v/@dev7a/otlp-stdout-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter) | Stable |

## Quick Comparison
{: .text-delta }

<div class="code-example" markdown="1">
{% capture rust_example %}
```rust
use otlp_stdout_client::StdoutClient;
use opentelemetry_otlp::WithExportConfig;

let exporter = opentelemetry_otlp::SpanExporter::builder()
    .with_http()
    .with_http_client(StdoutClient::default())
    .build()?;
```
{% endcapture %}

{% capture python_example %}
```python
from otlp_stdout_adapter import StdoutAdapter
from opentelemetry.exporter.otlp.proto.http.trace_exporter import OTLPSpanExporter

exporter = OTLPSpanExporter(
    session=StdoutAdapter().get_session(),
    timeout=5
)
```
{% endcapture %}

{% capture nodejs_example %}
```javascript
const { StdoutOTLPExporterNode } = require('@dev7a/otlp-stdout-exporter');

const exporter = new StdoutOTLPExporterNode({
  compression: 'gzip',
  format: 'protobuf'
});
```
{% endcapture %}

{: .tab-group }
<div class="tab rust active" markdown="1">
**Rust**
{{ rust_example }}
</div>
<div class="tab python" markdown="1">
**Python**
{{ python_example }}
</div>
<div class="tab nodejs" markdown="1">
**Node.js**
{{ nodejs_example }}
</div>
</div>

## Features by Language
{: .text-delta }

### Common Features
{: .text-delta }

{: .highlight }
All language implementations provide:
- OTLP format support (protobuf and JSON)
- Compression options (gzip)
- Automatic AWS Lambda context integration
- Configurable through environment variables
- Batch processing capabilities

### Language-Specific Features
{: .text-delta }

{: .success }
**Rust**
- Full async support with Tokio
- Zero-copy serialization
- Comprehensive error handling
- Memory-efficient implementation

{: .success }
**Python**
- Sync and async support
- Context propagation
- Integration with popular frameworks
- Automatic resource detection

{: .success }
**Node.js**
- Promise-based API
- TypeScript support
- Express/Fastify middleware
- Custom attribute processors

## Environment Variables
{: .text-delta }

Common configuration options across all languages:

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | `http/protobuf` or `http/json` | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | `gzip` or `none` | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service | Function name |

{: .info }
> See language-specific pages for additional configuration options.

## Coming Soon
{: .text-delta }

{: .warning }
We're working on support for:
- Java
- .NET
- Go

Want to contribute? Check out our [Contributing Guide](../contributing) to help add support for more languages! 