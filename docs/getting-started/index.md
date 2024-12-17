---
layout: default
title: Getting Started
nav_order: 2
has_children: true
---

# Getting Started
{: .fs-9 }

Installation and deployment guide for Serverless OTLP Forwarder.
{: .fs-6 .fw-300 }

{: .info }
> For OpenTelemetry concepts and terminology, refer to the [OpenTelemetry documentation](https://opentelemetry.io/docs/).

## Prerequisites
{: .text-delta }
<ol>
<li>Install required tools:

<div class="code-example" markdown="1">
{% capture macos_install %}
```bash
# Install AWS SAM CLI
brew install aws-sam-cli

# Install AWS CLI (if not installed)
brew install awscli

# Install rust and cargo-lambda
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-lambda

# Verify installations
sam --version
aws --version
```
{% endcapture %}

{% capture linux_install %}
```bash
# Install AWS SAM CLI
pip install aws-sam-cli

# Install AWS CLI (if not installed)
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install

# Install rust and cargo-lambda
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
cargo install cargo-lambda

# Verify installations
sam --version
aws --version
```
{% endcapture %}

{% capture windows_install %}
```powershell
# Install AWS SAM CLI
choco install aws-sam-cli

# Install AWS CLI (if not installed)
choco install awscli

# Download and run rustup-init.exe from https://rustup.rs/
# Install rust and cargo-lambda
cargo install cargo-lambda

# Verify installations
sam --version
aws --version
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
Configure the collector endpoint by creating a secret in AWS Secrets Manager:

```bash
aws secretsmanager create-secret \
  --name "serverless-otlp-forwarder/keys/default" \
  --secret-string '{
    "name": "my-collector",
    "endpoint": "https://collector.example.com",
    "auth": "x-api-key=your-api-key"
  }'
```
</li>

<li markdown="1">
Deploy the forwarder and demo stack:

```bash
# Clone repository
git clone https://github.com/dev7a/serverless-otlp-forwarder
cd serverless-otlp-forwarder

# Build and validate
sam build
sam validate --lint

# Deploy with guided setup
sam deploy --guided
```

The `--guided` flag initiates an interactive deployment process. When prompted:
- Confirm deployment of the demo stack
- Accept the creation of a function URL without authentication (can be removed after testing)
</li>

<li>Verify the deployment by checking your observability backend for demo stack traces.</li>
</ol>

## Implementation
{: .text-delta }

Choose your programming language and follow the corresponding guide to instrument your application:

- <i class="devicon-rust-plain colored"></i> [Rust Development Guide](../languages/rust)
- <i class="devicon-python-plain colored"></i> [Python Development Guide](../languages/python)
- <i class="devicon-nodejs-plain colored"></i> [Node.js Development Guide](../languages/nodejs)

## Configuration Options
{: .text-delta }

The forwarder supports various configuration options:
- [Basic Configuration](configuration) - Essential settings

