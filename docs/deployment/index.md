---
layout: default
title: Deployment
nav_order: 5
has_children: true
---

# Deployment Guide

The Lambda OTLP Forwarder is deployed using AWS SAM (Serverless Application Model). This section covers deployment options, configuration, and best practices.

## Deployment Options

- [Basic Deployment](basic-deployment): Standard setup with OTLP stdout processor
- [Application Signals](app-signals): Setup with AWS Application Signals processor
- [Multi-Account](multi-account): Deploying across multiple AWS accounts

## Configuration Options

### Core Parameters

- `ProcessorType` (Default: "otlp-stdout")
  - Selects which processor to deploy
  - Options: `otlp-stdout` or `aws-appsignals`

- `RouteAllLogs` (Default: "true")
  - Controls automatic log routing
  - Only applies to OTLP stdout processor

### Optional Features

- `DeployDemo` (Default: "true")
  - Deploys example applications
  - Useful for testing and validation

- `DeployBenchmark` (Default: "false")
  - Deploys performance testing functions
  - Use only when needed

## Best Practices

1. **One Forwarder Per Account**
   - Deploy one forwarder instance per AWS account
   - Use AWS Organizations for management

2. **Security**
   - Store collector credentials in Secrets Manager
   - Use IAM roles appropriately
   - Enable encryption in transit

3. **Monitoring**
   - Monitor forwarder performance
   - Set up alerts for failures
   - Track costs and usage

4. **Cost Optimization**
   - Use compression when possible
   - Configure appropriate log retention
   - Monitor CloudWatch Logs usage 