# OTLP Stdout Span Exporter Examples

This directory contains example programs demonstrating how to use the `otlp-stdout-span-exporter` crate in different output modes.

## Examples

### 1. `simple-stdout-hello.rs`

**Description:**
- Exports OpenTelemetry spans directly to standard output (stdout) in OTLP format.

**How to run:**
```sh
cargo run --example simple-stdout-hello
```

**Expected output:**
- Span data will be printed to stdout as JSON lines.

---

### 2. `simple-pipe-hello.rs`

**Description:**
- Exports OpenTelemetry spans to a named pipe (`/tmp/otlp-stdout-span-exporter.pipe`) in OTLP format.
- A background thread reads from the pipe and prints the output to stdout.

**How to run:**
```sh
cargo run --example simple-pipe-hello
```

**Notes:**
- The example will create the named pipe if it does not exist.
- Output will be printed to stdout by the reader thread.

---

For more details, see the source code of each example. 