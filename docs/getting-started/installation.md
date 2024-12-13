---
layout: default
title: Installation
parent: Getting Started
nav_order: 1
---

# Installation Guide
{: .fs-9 }

Step-by-step guide to install and set up Lambda OTLP Forwarder.
{: .fs-6 .fw-300 }

{: .warning }
Before you begin, ensure you have AWS credentials configured with appropriate permissions.

## Prerequisites
{: .text-delta }

### Required Tools
{: .text-delta }

<div class="code-example" markdown="1">
{% capture macos_tools %}
```bash
# Install Homebrew (if not installed)
/bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

# Install AWS SAM CLI
brew install aws-sam-cli

# Install AWS CLI
brew install awscli

# Verify installations
sam --version
aws --version
```
{% endcapture %}

{% capture linux_tools %}
```bash
# Install AWS SAM CLI
pip install aws-sam-cli

# Install AWS CLI
curl "https://awscli.amazonaws.com/awscli-exe-linux-x86_64.zip" -o "awscliv2.zip"
unzip awscliv2.zip
sudo ./aws/install

# Verify installations
sam --version
aws --version
```
{% endcapture %}

{% capture windows_tools %}
```powershell
# Install AWS SAM CLI
choco install aws-sam-cli

# Install AWS CLI
choco install awscli

# Verify installations
sam --version
aws --version
```
{% endcapture %}

{: .tab-group }
<div class="tab macos active" markdown="1">
**macOS**
{{ macos_tools }}
</div>
<div class="tab linux" markdown="1">
**Linux**
{{ linux_tools }}
</div>
<div class="tab windows" markdown="1">
**Windows**
{{ windows_tools }}
</div>
</div>

### Language-Specific Tools
{: .text-delta }

{: .highlight }
Install only what you need for your chosen language:

#### Rust Development
{: .text-delta }

```bash
# Install Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Install Cargo Lambda
cargo install cargo-lambda

# Verify installations
rustc --version
cargo lambda --version
```

#### Python Development
{: .text-delta }

```bash
# Install Python 3.12 (recommended)
# macOS
brew install python@3.12

# Linux
sudo apt install python3.12

# Verify installation
python3 --version
pip3 --version
```

#### Node.js Development
{: .text-delta }

```bash
# Install Node.js 18.x (LTS)
# Using nvm (recommended)
curl -o- https://raw.githubusercontent.com/nvm-sh/nvm/v0.39.0/install.sh | bash
nvm install 18
nvm use 18

# Verify installation
node --version
npm --version
```

## AWS Configuration
{: .text-delta }

### Configure AWS Credentials
{: .text-delta }

```bash
aws configure
```

{: .info }
Required AWS permissions:
```json
{
    "Version": "2012-10-17",
    "Statement": [
        {
            "Effect": "Allow",
            "Action": [
                "cloudformation:*",
                "lambda:*",
                "logs:*",
                "iam:*",
                "s3:*"
            ],
            "Resource": "*"
        }
    ]
}
```

### Region Setup
{: .text-delta }

```bash
# Set default region
aws configure set region us-west-2

# Verify configuration
aws configure list
```

## Installation Steps
{: .text-delta }

### 1. Clone Repository
{: .text-delta }

```bash
git clone https://github.com/dev7a/lambda-otlp-forwarder
cd lambda-otlp-forwarder
```

### 2. Build Project
{: .text-delta }

```bash
# Build SAM application
sam build

# Validate template
sam validate
```

### 3. Deploy
{: .text-delta }

```bash
# Interactive deployment
sam deploy --guided

# Or use configuration file
sam deploy --config-file samconfig.toml
```

### 4. Verify Installation
{: .text-delta }

```bash
# Check Lambda function
aws lambda list-functions | grep otlp-forwarder

# Check CloudWatch Log groups
aws logs describe-log-groups | grep otlp-forwarder

# Test function
aws lambda invoke \
  --function-name otlp-forwarder \
  --payload '{"action": "test"}' \
  response.json
```

## Environment Setup
{: .text-delta }

### CloudWatch Logs
{: .text-delta }

```bash
# Create log group (if needed)
aws logs create-log-group \
  --log-group-name /aws/lambda/otlp-forwarder

# Set retention policy
aws logs put-retention-policy \
  --log-group-name /aws/lambda/otlp-forwarder \
  --retention-in-days 7
```

### IAM Roles
{: .text-delta }

```bash
# Verify IAM role
aws iam get-role \
  --role-name otlp-forwarder-role

# Check role policies
aws iam list-role-policies \
  --role-name otlp-forwarder-role
```

## Troubleshooting
{: .text-delta }

### Common Issues
{: .text-delta }

{: .warning }
1. **AWS Credentials**
   - Check `~/.aws/credentials`
   - Verify permissions
   - Test with `aws sts get-caller-identity`

2. **Build Errors**
   - Clear SAM cache: `sam cache purge`
   - Update SAM CLI: `brew upgrade aws-sam-cli`
   - Check Python version

3. **Deployment Failures**
   - Review CloudFormation events
   - Check IAM permissions
   - Validate template syntax

### Validation
{: .text-delta }

```bash
# Validate SAM template
sam validate

# Validate CloudFormation
aws cloudformation validate-template \
  --template-body file://template.yaml

# Check Lambda configuration
aws lambda get-function-configuration \
  --function-name otlp-forwarder
```

## Next Steps
{: .text-delta }

- [Configure the Forwarder](configuration)
- [Set up Language SDKs](../languages)
- [Understand Architecture](../concepts/architecture)
- [View Advanced Features](../advanced) 