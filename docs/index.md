---
layout: home
title: Home
nav_order: 1
permalink: /
---

# Lambda OTLP Forwarder
{: .fs-9 }

The Lambda OTLP Forwarder enables serverless applications to send OpenTelemetry data to collectors without the overhead of direct connections or sidecars.
{: .fs-6 .fw-300 }

[Get Started](getting-started){: .btn .btn-primary .fs-5 .mb-4 .mb-md-0 .mr-2 }
[View on GitHub](https://github.com/dev7a/lambda-otlp-forwarder){: .btn .fs-5 .mb-4 .mb-md-0 }

---

{: .warning }
> This project is under active development. APIs and features may change.

![Architecture Diagram](https://github.com/user-attachments/assets/961999d9-bb69-4ba7-92a2-9efef3909b74)
{: .text-center }

## Quick Links
{: .text-delta }

- [Getting Started Guide](getting-started) - Set up and deploy in minutes
- [Architecture Overview](concepts/architecture) - Understand how it works
- [Configuration Guide](deployment/configuration) - Configure for your needs
- [Language Support](languages) - Choose your language

## Key Features
{: .text-delta }

- 🚀 **Reduced Latency**
Minimal impact on Lambda execution and cold start times
- 🔒 **Enhanced Security**
Keeps telemetry data within AWS infrastructure
- 💰 **Cost Optimization**
Supports compression and efficient protocols
- 🔄 **Multiple Languages**
Support for Rust, Python, and Node.js
- 📊 **AWS Application Signals**
Experimental support for AWS Application Signals

## TL;DR
{: .text-delta }

Tired of slow Lambda functions because of telemetry overhead? We've got you covered! 🚀

Lambda OTLP Forwarder makes sending OpenTelemetry data from your AWS Lambda functions super easy and efficient. Here's how it works:

1. Drop in our lightweight libraries for your favorite language (Rust, Python, or Node.js)
2. Add a few lines of instrumentation code
3. Let the magic happen - we'll capture your telemetry data from CloudWatch Logs and send it to your collector

That's it! No more dealing with slow extension layers or expensive direct connections. Your functions stay fast (just writing to stdout!) and your wallet stays happy. Plus, your architecture stays clean and simple.

Think of it as a smart pipeline that gets your telemetry data where it needs to go, without getting in your way. Pretty neat, right? 😊


## Getting Started

Check out our [Quick Start Guide](getting-started) to begin using the Lambda OTLP Forwarder.

---

## Support

Need help? Check out our:
- [Documentation](getting-started)
- [GitHub Issues](https://github.com/dev7a/lambda-otlp-forwarder/issues)
- [Troubleshooting Guide](troubleshooting)