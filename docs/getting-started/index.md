---
layout: default
title: Getting Started
nav_order: 2
has_children: true
---

# Getting Started
{: .fs-9 }

Get up and running with Lambda OTLP Forwarder in minutes.
{: .fs-6 .fw-300 }

{: .info }
> New to OpenTelemetry? Check out the [OpenTelemetry documentation](https://opentelemetry.io/docs/) first.

## Quick Start
{: .text-delta }

<ol>
<li>Install prerequisites:

<div class="code-example" markdown="1">
{% capture macos_install %}
```bash
# Install AWS SAM CLI
brew install aws-sam-cli
# Install rust and cargo-lambda
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-lambda
```
{% endcapture %}

{% capture linux_install %}
```bash
# Install AWS SAM CLI
pip install aws-sam-cli
# Install rust and cargo-lambda
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-lambda
```
{% endcapture %}

{% capture windows_install %}
```powershell
# Install AWS SAM CLI
choco install aws-sam-cli
# Download and run rustup-init.exe from https://rustup.rs/
# Install rust and cargo-lambda
cargo install cargo-lambda
```
{% endcapture %}

{: .tab-group }
<div class="tab macos active" markdown="1">
**macOS**
{{ macos_install }}
</div>
<div class="tab linux" markdown="1">
**Linux**
{{ linux_install }}
</div>
<div class="tab windows" markdown="1">
**Windows**
{{ windows_install }}
</div>
</div>
</li>

<li markdown="1">
Create a default secret in AWS Secrets Manager to define your collector or observability backend endpoint. Replace the `endpoint` and `auth` values with your collector endpoint and API key (if any).

```bash
aws secretsmanager create-secret \
  --name "lambda-otlp-forwarder/keys/default" \
  --secret-string '{
    "name": "my-collector",
    "endpoint": "https://collector.example.com",
    "auth": "x-api-key=your-api-key"
  }'
```
</li>

<li markdown="1">
Deploy the forwarder and the demo stack:

```bash
git clone https://github.com/dev7a/lambda-otlp-forwarder
cd lambda-otlp-forwarder
sam build && sam deploy --guided
```

With the `--guided` flag, SAM CLI will prompt you for the necessary parameters. 
Make sure that you are deploying the demo stack. You will have to reply `y` to the question about deploying a function url without authentication (you can delete the demo stack after testing).
</li>

<li>Open your observability backend and you should see traces from the demo stack.</li>
</ol>


## What's Next?
{: .text-delta }

After completing the quick start:
- Learn about the [Architecture](../concepts/architecture)
- Configure [Advanced Features](../advanced)
- Set up [Monitoring](../deployment/monitoring)

## Instrument your application
{: .text-delta }
Choose your language and follow the development guide to enable your application to send traces to the forwarder:
- <i class="devicon-rust-plain colored"></i> [Rust Development Guide](../languages/rust)
- <i class="devicon-python-plain colored"></i> [Python Development Guide](../languages/python)
- <i class="devicon-nodejs-plain colored"></i> [Node.js Development Guide](../languages/nodejs)


