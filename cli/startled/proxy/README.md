# startled-proxy

This is a generic Rust-based proxy Lambda function for use with the [`startled`](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/cli/startled) benchmarking tool.

## Purpose

The primary purpose of this proxy is to provide accurate, server-side measurement of another Lambda function's invocation time. When you use [`startled`](https://github.com/dev7a/serverless-otlp-forwarder/tree/main/cli/startled) to benchmark a function, you can instruct it to invoke this proxy instead of the target function directly. The proxy will then invoke the target function and measure the duration of that specific call, returning the result.

This isolates the performance of the target function from any client-side latency or overhead from the `startled` CLI itself, which is especially important for accurately measuring cold starts.
The proxy though is generic enough and can be used for other purposes as well, in all situations where you don't want to make any measurements on the client side depending on your local network conditions.


## Usage

1.  Deploy this application from the AWS Serverless Application Repository.
2.  When deploying, you can configure the `FunctionName` parameter. It defaults to `startled-proxy`.
3.  Take note of the function name you chose.
4.  Use the `--proxy-function` argument when running a benchmark with `startled`:

```bash
# Make sure to use the function name you specified during deployment
startled stack-benchmark \
  --stack-name your-test-stack \
  --proxy-function your-proxy-function-name \
  --select-pattern "your-function-substring"
```

## Parameters

- **FunctionName** (String, Default: `startled-proxy`): The name of the proxy Lambda function to be created. This is useful to avoid naming collisions if you deploy the proxy to multiple regions or for different purposes.

### What's New in v0.1.1
- **Enhanced Security Controls:** Introduced optional IAM condition keys (`TargetFunctionQualifier`, `PrincipalOrgID`) to provide a defense-in-depth security posture, allowing for much more granular control over which functions can be invoked.

- **TargetFunctionResource** (String, Default: `*`): An IAM resource pattern for the functions this proxy is allowed to invoke. Defaults to `*` for maximum flexibility. For a more secure posture, you can restrict this to a specific function ARN (e.g., `arn:aws:lambda:us-east-1:123456789012:function:my-function`) or a pattern (e.g., `arn:aws:lambda:us-east-1:123456789012:function:my-app-*`).

### Advanced Security Parameters

- **TargetFunctionResource** (String, Default: `*`): Restricts which functions can be invoked by ARN. For a secure posture, restrict this to a specific function ARN (e.g., `arn:aws:lambda:us-east-1:123456789012:function:my-function`) or a pattern (e.g., `arn:aws:lambda:us-east-1:123456789012:function:my-app-*`). To restrict to specific versions or aliases, include them in the ARN (e.g., `arn:aws:lambda:us-east-1:123456789012:function:my-function:prod` or `arn:aws:lambda:us-east-1:123456789012:function:my-function:1`).
- **PrincipalOrgID** (String, Default: *empty*): If you use AWS Organizations, you can provide your Organization ID here. This will restrict the proxy to only invoke functions in accounts that are part of your organization.

## API Reference

The proxy function expects a specific JSON structure for its input event and returns a JSON object with a consistent format.

### Request Format

The input event payload must be a JSON object with the following structure:

```json
{
  "target": "string",
  "payload": "any"
}
```

- **target** (String, Required): The name or ARN of the Lambda function that the proxy should invoke.
- **payload** (Any, Required): The JSON payload that will be passed directly to the `target` function. The proxy also inspects this payload for an `X-Amzn-Trace-Id` header (either in `payload.headers.X-Amzn-Trace-Id` or `payload.X-Amzn-Trace-Id`) and propagates it if found to ensure the trace context is continued.

### Response Format

The proxy function returns a JSON object with the following structure:

```json
{
  "invocation_time_ms": "number",
  "response": "any"
}
```

- **invocation_time_ms** (Number): A floating-point number representing the total time taken for the proxy to invoke the target function and receive a response, measured in milliseconds. This is the server-side duration.
- **response** (Any): The complete, unmodified JSON payload returned by the `target` function.

## Permissions

This function includes an IAM policy that grants it `lambda:InvokeFunction` permissions. For improved security, you can use the parameters described above to create a defense-in-depth policy. The policy will always enforce that the invocation type is `RequestResponse`, and will additionally apply restrictions for `Resource`, `Qualifier`, and `PrincipalOrgID` if you provide them.

When deploying from SAR, you will be asked to acknowledge the creation of the IAM role.

## Development

To build and deploy the proxy function from the source code, you can use the AWS SAM CLI from within this directory (`cli/startled/proxy`).

### Build

```bash
sam build --template-file proxy-template.yaml
```

### Deploy

After building, you can deploy the function directly to your AWS account:

```bash
make deploy
```

### Publishing to SAR

This project is configured for automated publishing to the AWS Serverless Application Repository (SAR) using the provided `Makefile`.

To perform a full release, first ensure the `VERSION` file in this directory contains the correct target version. You can manually edit it or use the `make bump-patch` command to increment the patch version.

Then, run the release command:

```bash
make release
```

This single command handles all the steps that were previously done manually. You can also run the steps individually (`build`, `package`, `publish`, `public`) if needed. 