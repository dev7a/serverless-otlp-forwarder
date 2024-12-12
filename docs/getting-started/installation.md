---
layout: default
title: Installation
parent: Getting Started
nav_order: 1
---

# Installation Guide

This guide walks you through installing and setting up the Lambda OTLP Forwarder and its language-specific libraries.

## Prerequisites

1. **AWS SAM CLI**
   ```bash
   # macOS
   brew install aws-sam-cli

   # Linux/WSL
   pip install aws-sam-cli

   # Windows
   choco install aws-sam-cli
   ```

2. **Language-Specific Tools** (install only what you need)

   **Rust:**
   ```bash
   curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
   cargo install cargo-lambda
   ```

   **Python:**
   ```bash
   # Python 3.12+ is recommended
   python -m pip install --upgrade pip
   ```

   **Node.js:**
   ```bash
   # Node.js 18.x is recommended
   nvm install 18
   nvm use 18
   ```

## Installing Language Libraries

### Rust
Add to your `Cargo.toml`:
```toml
[dependencies]
otlp-stdout-client = "0.2.1"
```
Or use:
```bash
cargo add otlp-stdout-client
```

### Python
Install using pip:
```bash
pip install otlp-stdout-adapter
```

### Node.js
Install using npm:
```bash
npm install @dev7a/otlp-stdout-exporter @opentelemetry/api @opentelemetry/sdk-trace-node
```

## Deploying the Forwarder

1. Clone the repository:
   ```bash
   git clone https://github.com/dev7a/lambda-otlp-forwarder
   cd lambda-otlp-forwarder
   ```

2. Build and deploy:
   ```bash
   sam build
   sam deploy --guided
   ```

   During the guided deployment, you'll be asked to configure:
   - Stack name
   - AWS Region
   - Processor type
   - Log routing options
   - Demo application deployment

## Verifying Installation

1. Check AWS CloudFormation for successful stack creation
2. Verify Lambda function deployment
3. Test with the demo application (if deployed)

## Next Steps

- [Configure your application](configuration)
- [Set up your first instrumented function](first-application)
- [Learn about the architecture](../concepts/architecture) 