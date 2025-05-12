# Release Notes - startled v0.3.0

**Release Date:** 2025-05-12

This release introduces parallel benchmarking for stack commands and makes the `--memory` option required for all benchmarks.

## Key Highlights

- **Parallel Stack Benchmarks:** The new `--parallel` option for the `stack` command allows benchmarking all selected Lambda functions concurrently, with an overall progress bar and summary output.
- **Required Memory Argument:** The `--memory` option is now required for both `function` and `stack` commands, simplifying result directory structures and ensuring explicit configuration.
- **Improved Output:** Console output for parallel stack benchmarks is now cleaner, with configuration printing serialized and verbose logs suppressed for individual function tasks.
