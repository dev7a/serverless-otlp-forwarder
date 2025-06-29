# Makefile for managing the startled-proxy serverless application.

# Default variables. Can be overridden from the command line.
# Example: make release REGION=us-west-2
# Example: make deploy PARAMETERS_OVERRIDE="TargetFunctionResource=arn:aws:lambda:us-east-1:123456789012:function:my-app-*"
AWS_REGION  ?= us-east-1
# This must match the 'Name' property in template.yaml's Metadata section.
APPLICATION_NAME := startled-proxy
TEMPLATE    ?= template.yaml
PACKAGED_TEMPLATE := packaged.yaml
SEMVER := $(shell cat VERSION)
# Optional parameter overrides for testing (e.g., "Key1=Value1 Key2=Value2")
PARAMETERS_OVERRIDE ?=

# Get AWS Account ID automatically.
AWS_ACCOUNT_ID := $(shell aws sts get-caller-identity --query Account --output text)
APPLICATION_ID := arn:aws:serverlessrepo:$(AWS_REGION):$(AWS_ACCOUNT_ID):applications/$(APPLICATION_NAME)

# Phony targets don't represent files.
.PHONY: all build package publish public release deploy clean help bump-patch

all: help

help:
	@echo "Usage: make <target>"
	@echo ""
	@echo "Targets:"
	@echo "  build         Build the Lambda function using 'sam build'."
	@echo "  package       Package the application artifacts to S3 (reads bucket from samconfig.toml)."
	@echo "  publish       Publish the packaged application to SAR (as private)."
	@echo "  public        Make the latest published version of the application public."
	@echo "  release       Run all steps: build, package, publish, and make it public."
	@echo "  bump-patch    Increment the patch number in the VERSION file."
	@echo "  deploy        Deploy the stack directly to your account for development/testing."
	@echo "  clean         Remove build artifacts and the packaged template."
	@echo ""
	@echo "Examples:"
	@echo "  make deploy"
	@echo "  make deploy PARAMETERS_OVERRIDE=\"TargetFunctionResource=arn:aws:lambda:*:*:function:my-app-*\""
	@echo "  make deploy PARAMETERS_OVERRIDE=\"TargetFunctionResource=arn:aws:lambda:*:*:function:my-function:prod PrincipalOrgID=o-123456789\""

build:
	@echo "--> Building SAM application..."
	@sam build --template-file $(TEMPLATE)

package: build
	@echo "--> Packaging application..."
	@sam package \
		--output-template-file $(PACKAGED_TEMPLATE) \
		--force-upload

publish: package
	@echo "--> Publishing application '$(APPLICATION_NAME)' with version $(SEMVER) to SAR..."
	@sam publish \
		--template $(PACKAGED_TEMPLATE) \
		--region $(AWS_REGION) \
		--semantic-version $(SEMVER)

public:
	@echo "--> Making application $(APPLICATION_ID) public..."
	@aws serverlessrepo put-application-policy \
		--application-id $(APPLICATION_ID) \
		--statements Principals='*',Actions='Deploy' \
		--region $(AWS_REGION)
	@echo "Application is now public."

release: publish public

# For local development/testing, not part of the SAR release workflow.
deploy: build
	@echo "--> Deploying stack for development (using settings from samconfig.toml)..."
	@if [ -n "$(PARAMETERS_OVERRIDE)" ]; then \
		echo "Using parameter overrides: $(PARAMETERS_OVERRIDE)"; \
		sam deploy --parameter-overrides $(PARAMETERS_OVERRIDE); \
	else \
		sam deploy; \
	fi

clean:
	@echo "--> Cleaning up build artifacts..."
	@rm -rf .aws-sam $(PACKAGED_TEMPLATE)

bump-patch:
	@echo "Bumping patch version..."
	@python3 -c "with open('VERSION', 'r+') as f: v = f.read().strip().split('.'); v[2] = str(int(v[2]) + 1); new_v = '.'.join(v); f.seek(0); f.write(new_v); f.truncate()"
	@echo "New version: $$(cat VERSION)"
