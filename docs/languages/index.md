---
layout: default
title: Language Support
nav_order: 4
has_children: true
---

# Language Support
{: .fs-9 }

Language-specific packages for Serverless OTLP Forwarder integration.
{: .fs-6 .fw-300 }

## Supported Languages
{: .text-delta }

| Language | Package | Version | Status |
|:---------|:--------|:--------|:-------|
| [Rust](rust) | `otlp-stdout-client` | [![Crates.io](https://img.shields.io/crates/v/otlp-stdout-client.svg)](https://crates.io/crates/otlp-stdout-client) | Alpha |
| [Python](python) | `otlp-stdout-adapter` | [![PyPI](https://img.shields.io/pypi/v/otlp-stdout-adapter.svg)](https://pypi.org/project/otlp-stdout-adapter/) | Alpha |
| [Node.js](nodejs) | `@dev7a/otlp-stdout-exporter` | [![npm](https://img.shields.io/npm/v/@dev7a/otlp-stdout-exporter.svg)](https://www.npmjs.com/package/@dev7a/otlp-stdout-exporter) | Alpha |

## Integration Overview
{: .text-delta }

Each language package provides a lightweight adapter that:
- Serializes OTLP data to stdout
- Supports both JSON and protobuf formats
- Handles compression when configured
- Integrates with standard OpenTelemetry SDKs

## Quick Examples
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

## Configuration
{: .text-delta }

All language implementations support standard OpenTelemetry environment variables:

| Variable | Description | Default |
|:---------|:------------|:--------|
| `OTEL_EXPORTER_OTLP_PROTOCOL` | Protocol for OTLP data (`http/protobuf` or `http/json`) | `http/protobuf` |
| `OTEL_EXPORTER_OTLP_COMPRESSION` | Compression type (`gzip` or `none`) | `gzip` |
| `OTEL_SERVICE_NAME` | Name of your service for telemetry identification | - |

{: .note }
See the language-specific guides for detailed integration instructions and examples. 