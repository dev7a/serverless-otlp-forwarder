# OTLP Stdout livetrace

`livetrace` is a command-line tool designed to enhance local development workflows when working with distributed tracing in serverless environments using the [Serverless OTLP Forwarder Architecture](https://dev7a.github.io/serverless-otlp-forwarder/architecture/).

## Table of Contents

*   [Overview](#overview)
*   [Features](#features)
*   [Installation](#installation)
    *   [Prerequisites](#prerequisites)
    *   [From Crates.io (Recommended)](#from-cratesio-recommended)
    *   [From Source](#from-source)
*   [Usage](#usage)
    *   [Discovery Options](#discovery-options)
    *   [Mode and Duration Control](#mode-and-duration-control)
    *   [OTLP Forwarding](#otlp-forwarding)
    *   [Console Display Options](#console-display-options)
    *   [Other Options](#other-options)
*   [Console Output](#console-output)
*   [Configuration Profiles](#configuration-profiles)
    *   [Saving a Profile](#saving-a-profile)
    *   [Using a Profile](#using-a-profile)
    *   [Configuration File Format](#configuration-file-format)
*   [Shell Completions](#shell-completions)
    *   [Installation Examples](#installation-examples)
*   [Development](#development)
*   [License](#license)

## Overview

[livetracing a demo app](https://github.com/user-attachments/assets/9087fbbe-3e99-47d2-8cb3-64273eea5204)

In the [Serverless OTLP Forwarder architecture](https://dev7a.github.io/serverless-otlp-forwarder/architecture/), Lambda functions (or other compute resources) emit OpenTelemetry (OTLP) trace data to standard output. This tool enables you to correlate and visualize complete traces—especially valuable during development. Because logs from different services involved in a single request may be distributed across multiple Log Groups, _livetrace_ can tail several log groups simultaneously and reconstruct traces spanning all participating services.

`livetrace` supports:

1.  **Discovering** relevant CloudWatch Log Groups based on naming patterns or CloudFormation stack resources.
2.  **Validating** the existence of these Log Groups, intelligently handling standard Lambda and Lambda@Edge naming conventions.
3.  **Tailing** or **Polling** these Log Groups simultaneously using either the `StartLiveTail`or the `FilterLogEvents` APIs.
4.  **Parsing** OTLP trace data embedded within log messages in the format produced by the _otlp-stdout-span-exporter_ ([npm](https://www.npmjs.com/package/@dev7a/otlp-stdout-span-exporter), [pypi](https://pypi.org/project/otlp-stdout-span-exporter/), [crates.io](https://crates.io/crates/otlp-stdout-span-exporter)).
5.  **Displaying** traces in a user-friendly waterfall view directly in your terminal, including service names, durations, and timelines.
6.  **Showing** span events associated with the trace.
7.  **Optionally forwarding** the raw OTLP protobuf data to a specified OTLP-compatible endpoint (like a local OpenTelemetry Collector or Jaeger instance).

It acts as a local observability companion, giving you immediate feedback on trace behavior without needing to navigate the AWS console, your o11y tool, or wait for logs to propagate fully to a backend system.

To instrument your lambda functions, you can use the OTLP stdout span exporter, available for Node, Python, and Rust:

*   [npm](https://www.npmjs.com/package/@dev7a/otlp-stdout-span-exporter)
*   [pypi](https://pypi.org/project/otlp-stdout-span-exporter/)
*   [crates.io](https://crates.io/crates/otlp-stdout-span-exporter)

Or, you can use the Lambda Otel Lite library, which also simplifies setting up your OpenTelemetry pipeline:

*   [npm](https://www.npmjs.com/package/@dev7a/lambda-otel-lite)
*   [pypi](https://pypi.org/project/lambda-otel-lite/)
*   [crates.io](https://crates.io/crates/lambda-otel-lite)





## Features

*   **CloudWatch Log Tailing:** Stream logs in near real-time using `StartLiveTail`.
*   **CloudWatch Log Polling:** Periodically fetch logs using `FilterLogEvents` with `--poll-interval`.
*   **Flexible Log Group Discovery:**
    *   Find log groups matching one or more patterns (`--log-group-pattern`).
    *   Find log groups belonging to a CloudFormation stack (`--stack-name`), including implicitly created Lambda log groups.
    *   **Combine pattern and stack discovery:** Use both options simultaneously to aggregate log groups.
*   **Support for Lambda@Edge:** Checks existence and handles Lambda@Edge naming conventions (`/aws/lambda/<region>.<function-name>`).
*   **OTLP/stdout Parsing:** Decodes trace data logged via the `otlp-stdout-span-exporter` format (JSON wrapping base64-encoded, gzipped OTLP protobuf).
*   **Console Trace Visualization:**
    *   Waterfall view showing span hierarchy, service names, durations, and relative timing.
    *   Display of span kind (SERVER, CLIENT, etc.) and important span attributes in the waterfall.
    *   Configurable color themes for better service differentiation (`--theme`, `--list-themes`).
*   **Console Event Display:** Lists span events with timestamps, service names, and optional attribute filtering including both event and related span attributes.
*   **OTLP Forwarding:** Optionally send processed trace data to an OTLP HTTP endpoint (`-e`, `-H`, Environment Variables).
*   **Configuration:**
    *   AWS Region/Profile support.
    *   OTLP endpoint and headers configurable via CLI args or standard OTel environment variables.
    *   Session timeout for Live Tail mode (`--session-timeout`).
    *   Configuration profiles (`.livetrace.toml`) for saving common settings (`--config-profile`, `--save-profile`).
*   **User Experience:**
    *   **Detailed Startup Preamble:** Shows a summary of the effective configuration (AWS details, discovery sources, modes, forwarding, display settings, log groups).
    *   **Interactive Spinner:** Displays a spinner while waiting for events.
    *   **Verbosity Control:** Adjust logging detail (`-v`, `-vv`, `-vvv`).

## Installation

### Prerequisites

*   Rust toolchain (latest stable recommended). You can install it from [rustup.rs](https://rustup.rs/).
*   AWS Credentials configured (via environment variables, shared credentials file, etc.) accessible to the tool, with permissions to read CloudWatch Logs and, if using stack discovery, CloudFormation resources.

### From Crates.io (Recommended)

```bash
cargo install livetrace
```

### From Source

If you want to build from the latest source code or contribute to development:

1.  **Clone the repository:**
    ```bash
    git clone https://github.com/dev7a/serverless-otlp-forwarder.git
    cd serverless-otlp-forwarder
    ```

2.  **Build and install the `livetrace` binary:**
    ```bash
    cargo install --path cli/livetrace
    ```
    This will compile the `livetrace` crate and place the binary in your Cargo bin directory (e.g., `~/.cargo/bin/livetrace`). Ensure this directory is in your system's `PATH`.

## Usage

```bash
livetrace [OPTIONS]
```

### Discovery Options

You must specify at least one of the following to identify the log groups. They can be used together:

*   `--log-group-pattern <PATTERN>...`: Discover log groups whose names contain *any* of the given patterns (case-sensitive substring search). Can be specified multiple times, or provide multiple patterns after the flag.
    ```bash
    # Single pattern
    livetrace --log-group-pattern "/aws/lambda/my-app-"
    # Multiple patterns
    livetrace --log-group-pattern "/aws/lambda/service-a-" "/aws/lambda/service-b-"
    livetrace --log-group-pattern "pattern1" --log-group-pattern "pattern2"
    ```
*   `--stack-name <STACK_NAME>`: Discover log groups associated with resources (`AWS::Logs::LogGroup`, `AWS::Lambda::Function`) in the specified CloudFormation stack.
    ```bash
    livetrace --stack-name my-production-stack
    ```
*   **Combining:**
    ```bash
    # Find groups in a stack AND those matching a pattern
    livetrace --stack-name my-api-stack --log-group-pattern "/aws/lambda/auth-"
    ```

### Mode and Duration Control

*   `--poll-interval <DURATION>`: Use the `FilterLogEvents` API instead of `StartLiveTail`, polling at the specified interval. Duration format requires a unit suffix (e.g., `10s`, `1500ms`, `1m`). Decimal values are not supported. If this option is not provided, Live Tail mode is used by default.
    ```bash
    # Poll every 15 seconds
    livetrace --stack-name my-dev-stack --poll-interval 15s
    ```
*   `--backtrace <DURATION>`: (Polling mode only) Fetch logs starting from `<DURATION>` ago for the initial poll. Duration format requires a unit suffix (e.g., `30s`, `5m`, `2h`). Decimal values are not supported. Subsequent polls fetch new logs.
    ```bash
    # Poll, fetching initial logs from the last 2 minutes
    livetrace --stack-name my-dev-stack --poll-interval 15s --backtrace 2m
    ```
*   `--session-timeout <DURATION>`: (Default: `30m`) Automatically exit after the specified duration. Applies to both Live Tail mode and Polling mode. Duration format requires a unit suffix (e.g., `30m`, `1h`, `900s`). Decimal values are not supported.
    ```bash
    # Use Live Tail, but exit after 60 minutes
    livetrace --pattern "my-service-" --session-timeout 60m
    # Poll every 10 seconds, but exit after 5 minutes total
    livetrace --stack-name my-app --poll-interval 10s --session-timeout 5m
    ```
[!NOTE]
> Live Tail mode is the default, but it's not free, at 1c/minute. For long sessions, it's probably better to use the `FilterLogEvents` API with a polling interval.

### OTLP Forwarding

Configure forwarding to send traces to another OTLP receiver:

*   `-e, --otlp-endpoint <URL>`: The base HTTP URL for the OTLP receiver (e.g., `http://localhost:4318`). `/v1/traces` will be appended automatically if no path is present.
*   `-H, --otlp-header <KEY=VALUE>`: Add custom HTTP headers (e.g., for authentication). Can be specified multiple times.

**Environment Variables for Forwarding:**

You can also configure the endpoint and headers using standard OpenTelemetry environment variables. The precedence order is:

1.  Command-line arguments (`-e`, `-H`)
2.  Signal-specific environment variables (`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, `OTEL_EXPORTER_OTLP_TRACES_HEADERS`)
3.  General OTLP environment variables (`OTEL_EXPORTER_OTLP_ENDPOINT`, `OTEL_EXPORTER_OTLP_HEADERS`)

*   `OTEL_EXPORTER_OTLP_ENDPOINT=<URL>` / `OTEL_EXPORTER_OTLP_TRACES_ENDPOINT=<URL>`: Base URL for the receiver.
*   `OTEL_EXPORTER_OTLP_HEADERS=<KEY1=VAL1,KEY2=VAL2...>` / `OTEL_EXPORTER_OTLP_TRACES_HEADERS=<KEY1=VAL1,KEY2=VAL2...>`: Comma-separated list of key-value pairs for headers.

```bash
# Forward using CLI args
livetrace --stack-name my-stack -e http://localhost:4318 -H "Authorization=Bearer mytoken"

# Forward using environment variables
export OTEL_EXPORTER_OTLP_ENDPOINT=http://collector:4318
export OTEL_EXPORTER_OTLP_HEADERS="x-api-key=secret123,x-tenant-id=abc"
livetrace --stack-name my-stack
```

### Console Display Options

Control the appearance of the console output:

*   `--theme <THEME>`: Select a color theme (e.g., `default`, `tableau`, `monochrome`). Default is `default`.
*   `--list-themes`: List all available color themes with descriptions and exit.
*   `--attrs <GLOB_LIST>`: Comma-separated list of glob patterns (e.g., `"http.*,db.statement,my.custom.*"`) to filter which attributes are displayed. Applied to both span attributes and event attributes. If omitted, all attributes are shown.
*   `--grep <REGEX>`: Filter entries in the **Timeline Log**. Only SpanStart and Event entries where at least one attribute *value* (including parent span attributes for events) matches the provided Rust-compatible regular expression will be shown. Matching text within attribute values will be highlighted (yellow background). This filter does not affect the waterfall span display.
    ```bash
    # Show only timeline log entries where an attribute value contains "error" or "failure"
    livetrace --pattern "my-app" --grep "error|failure"
    ```
*   `--color-by <MODE>`: Specify how spans are colored in the waterfall and timeline views.
    *   `service`: Color by service name.
    *   `span`: Color by span ID. (Default: `span`)
*   `--event-severity-attribute <ATTRIBUTE_NAME>`: (Default: `event.severity`) Specify the event attribute key used to determine the severity level for coloring event output.
*   `--events-only [true|false]`: Controls visibility of span start entries in the timeline log. By default (`true`), only events are shown. Use `--events-only=false` to include span start information. Providing the flag without a value (e.g., `--events-only`) implies `true`.
*   `--trace-timeout <DURATION>`: (Default: `5s`) Maximum time to wait for spans belonging to a trace before displaying/forwarding it. Duration format requires a unit suffix (e.g., `5s`, `500ms`, `1m`). Decimal values are not supported.
*   `--trace-stragglers-wait <DURATION>`: (Default: `500ms`) Time to wait for late-arriving (straggler) spans after the last observed activity on a trace (if its root span has been received) before flushing. Useful for collecting additional spans that might arrive slightly out of order. Duration format requires a unit suffix (e.g., `500ms`, `1s`). Decimal values are not supported.

### Other Options

*   `--aws-region <AWS_REGION>`: Specify the AWS Region. Defaults to environment/profile configuration.
*   `--aws-profile <AWS_PROFILE>`: Specify the AWS profile name.
*   `-v, -vv, -vvv`: Increase logging verbosity (Info -> Debug -> Trace). Internal logs go to stderr.
*   `--forward-only`: Only forward telemetry via OTLP; do not display traces/events in the console. Requires an endpoint to be configured.
*   `--config-profile <PROFILE_NAME>`: Load configuration from a named profile in `.livetrace.toml`.
*   `--save-profile <PROFILE_NAME>`: Save the current command-line arguments as a named profile in `.livetrace.toml`.

## Console Output

When running in console mode (`--forward-only` not specified), `livetrace` displays:

1.  **Configuration Preamble:** Shows a detailed summary of the effective configuration being used, including AWS details, discovery sources, mode, forwarding settings, display options, and the final list of log groups being monitored.
2.  **Spinner:** An animated spinner indicates when the tool is actively waiting for new log events.
3.  **Trace Waterfall:** For each trace received:
    *   A header `─ Trace ID: <trace_id> ───────────`
    *   A table showing:
        *   Service Name
        *   Span Name (indented based on parent-child relationship)
        *   Span Kind (SERVER, CLIENT, etc.)
        *   Status (OK, ERROR)
        *   Duration (ms)
        *   Span ID (shortened to 8 characters)
        *   Timeline bar visualization (colored based on --color-by setting)
4.  **Timeline Log:** For each trace received:
    *   A header `─ Timeline Log for Trace: <trace_id> ─────` (or `─ Events for Trace: <trace_id> ─────` if `--events-only` is used)
    *   A chronological list of span starts and events showing:
        *   Timestamp (colored dimmed)
        *   Span ID (shortened to 8 characters, colored based on --color-by setting)
        *   Service Name (in square brackets)
        *   Type tag (`[SPAN]` or `[EVENT]`) - `[SPAN]` entries are hidden if `--events-only` is used
        *   Status/Level (colored appropriately: green for OK, red for ERROR, etc.)
        *   Name (Span name or Event name)
        *   Attributes (if present): filtered by `--attrs` if provided

## Configuration Profiles

`livetrace` supports saving and loading configuration profiles to reduce typing for frequently used commands. Profiles are stored in a `.livetrace.toml` file in the current directory.

### Saving a Profile

To save your current command-line options as a named profile:

```bash
# Save the current settings as "dev-profile"
livetrace --pattern "my-service-" --attrs "http.*" --save-profile dev-profile
```

### Using a Profile

To use a saved profile:

```bash
# Load settings from the "dev-profile"
livetrace --config-profile dev-profile
```

You can also override specific settings from the profile by providing additional command-line arguments:

```bash
# Load from profile but override the attribute filter
livetrace --config-profile dev-profile --attrs "db.*,aws.*"
```

### Configuration File Format

The `.livetrace.toml` file follows this structure:

```toml
version = 0.0

# Global settings applied to all profiles
[global]
aws-region = "us-east-1"
event-severity-attribute = "event.severity"

# Profile-specific settings
[profiles.dev-profile]
log-group-pattern = ["my-service-"]
attrs = "http.*"
theme = "solarized"
events-only = true
trace-timeout = 10

[profiles.prod-profile]
stack-name = "production-stack"
forward-only = true
otlp-endpoint = "http://localhost:4318"
```

This file is meant to be local to your project or environment and should typically not be committed to version control.

## Shell Completions

`livetrace` can generate shell completion scripts for Bash, Elvish, Fish, PowerShell, and Zsh.
This allows you to get command-line suggestions by pressing the Tab key.

To generate a script, use the `generate-completions` subcommand:

```bash
livetrace generate-completions <SHELL>
```

Replace `<SHELL>` with your desired shell (e.g., `bash`, `zsh`, `fish`).

### Installation Examples

The exact installation method varies by shell. Here are some common examples:

**Bash:**

1.  Ensure you have `bash-completion` installed (often available via your system\'s package manager).
2.  Create the completions directory if it doesn\'t exist:
    ```bash
    mkdir -p ~/.local/share/bash-completion/completions
    ```
3.  Generate the script and save it:
    ```bash
    livetrace generate-completions bash > ~/.local/share/bash-completion/completions/livetrace
    ```
    You may need to restart your shell or source your `.bashrc` for changes to take effect.

**Zsh:**

1.  Create a directory for completions if you don\'t have one (e.g., `~/.zsh/completions`).
    ```bash
    mkdir -p ~/.zsh/completions
    ```
2.  Add this directory to your `fpath` in your `.zshrc` file *before* `compinit` is called:
    ```zsh
    # In ~/.zshrc
    fpath=(~/.zsh/completions $fpath)
    # ... (ensure compinit is called after this, e.g., autoload -U compinit && compinit)
    ```
3.  Generate the script:
    ```bash
    livetrace generate-completions zsh > ~/.zsh/completions/_livetrace
    ```
    You may need to restart your shell or run `compinit` again.

**Fish:**

1.  Create the completions directory if it doesn\'t exist:
    ```bash
    mkdir -p ~/.config/fish/completions
    ```
2.  Generate the script:
    ```bash
    livetrace generate-completions fish > ~/.config/fish/completions/livetrace.fish
    ```
    Fish should pick up the completions automatically on next launch.

Refer to your shell\'s documentation for the most up-to-date and specific instructions.

## Development

```bash
# Build
cargo build -p livetrace

# Run tests
cargo test -p livetrace

# Run clippy checks
cargo clippy -p livetrace -- -D warnings
```

## License

This project is licensed under the MIT License. See the [LICENSE](https://github.com/dev7a/serverless-otlp-forwarder/blob/main/cli/livetrace/LICENSE) file for details.
