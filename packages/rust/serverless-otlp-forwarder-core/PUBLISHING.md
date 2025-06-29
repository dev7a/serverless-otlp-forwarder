# Publishing Checklist

Before publishing a new version of `serverless-otlp-forwarder-core`, ensure all these items are checked:

## Cargo.toml Verification
- [ ] `version` is correctly incremented (following semver)
- [ ] `name` is correct
- [ ] `description` is clear and up-to-date
- [ ] `license` is specified
- [ ] `keywords` are defined and relevant
- [ ] `categories` are appropriate
- [ ] `repository` information is complete and correct
- [ ] `homepage` URL is valid
- [ ] `documentation` URL is specified
- [ ] Dependencies are up-to-date and correct (currently using OpenTelemetry 0.30.0)
- [ ] No extraneous dependencies
- [ ] Development dependencies are in `[dev-dependencies]`
- [ ] Feature flags are correctly defined (`instrumented-client`)
- [ ] Minimum supported Rust version (MSRV) is specified if needed

## Documentation
- [ ] `README.md` is up-to-date
- [ ] Version number in documentation matches Cargo.toml
- [ ] All examples in documentation work
- [ ] API documentation is complete (all public items have doc comments)
- [ ] Breaking changes are clearly documented
- [ ] `CHANGELOG.md` is updated
- [ ] Feature flags are documented
- [ ] All public APIs have usage examples
- [ ] All environment variables are documented (`OTEL_EXPORTER_OTLP_TRACES_ENDPOINT`, `OTEL_EXPORTER_OTLP_TRACES_HEADERS`, `OTEL_EXPORTER_OTLP_COMPRESSION`, `OTEL_EXPORTER_OTLP_COMPRESSION_LEVEL`)

## Code Quality
- [ ] All tests pass (`cargo test`)
- [ ] Code is properly formatted (`cargo fmt`)
- [ ] Format check passes (`cargo fmt --check`)
- [ ] Linting passes (`cargo clippy -- -D warnings`)
- [ ] No debug code or println! macros (except in logging)
- [ ] Test coverage is satisfactory (83+ tests currently)
- [ ] All public APIs have proper documentation
- [ ] No unsafe code (or if present, properly documented and justified)
- [ ] All compiler warnings are addressed
- [ ] Documentation tests (`cargo test --doc`) pass

## Git Checks
- [ ] Working on the correct branch
- [ ] All changes are committed
- [ ] No unnecessary files in git
- [ ] Git tags are ready to be created
- [ ] `.gitignore` is up-to-date

## Version Management
- [ ] Update version in `Cargo.toml` only (or in workspace Cargo.toml if using workspace version)
- [ ] This is the single source of truth for the version

## Publishing Steps
1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Run quality checks:
   ```bash
   cargo fmt
   cargo fmt --check
   cargo clippy -- -D warnings
   cargo test
   cargo test --doc
   ```
4. Build in release mode:
   ```bash
   cargo build --release
   cargo doc --no-deps
   cargo package # Verify package contents
   ```
5. Create a branch for the release following the pattern `release/rust/<package-name>-v<version>`
6. Commit changes to the release branch and push to GitHub, with a commit message of `release: rust/serverless-otlp-forwarder-core v<version>`
7. Create a Pull Request to merge your changes to the main branch
8. Once the PR is approved and merged, tagging and publishing is done automatically by the CI pipeline

## Post-Publishing
- [ ] Verify package installation works: `cargo add serverless-otlp-forwarder-core`
- [ ] Verify documentation appears correctly on docs.rs
- [ ] Test the package in a new project
- [ ] Update any dependent crates
- [ ] Verify examples compile and run correctly

## Common Issues to Check
- Missing files in the published package
- Incorrect feature flags
- Missing or incorrect documentation
- Broken links in documentation
- Incorrect version numbers
- Missing changelog entries
- Unintended breaking changes
- Incomplete crate metadata
- Platform-specific issues
- MSRV compatibility issues

## Notes
- Test the package with different feature combinations (default, instrumented-client)
- Consider cross-platform compatibility
- Test with the minimum supported Rust version
- Consider running `cargo audit` for security vulnerabilities
- Use `cargo clippy` with all relevant feature combinations
- Remember to update any related documentation or examples in the main repository
- Consider testing on different architectures (x86_64, aarch64)
- Ensure HTTP client implementations work correctly across different configurations
- Verify span compaction functionality works with various OTLP payloads
- Test environment variable precedence and configuration resolution
- Verify instrumented client feature works with middleware stacks 