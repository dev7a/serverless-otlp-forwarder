# Publishing Checklist

Before publishing a new version of `lambda-otel-lite`, ensure all these items are checked:

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
- [ ] Dependencies are up-to-date and correct
- [ ] No extraneous dependencies
- [ ] Development dependencies are in `[dev-dependencies]`
- [ ] Feature flags are correctly defined
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

## Code Quality
- [ ] All tests pass (`cargo test`)
- [ ] Code is properly formatted (`cargo fmt`)
- [ ] Format check passes (`cargo fmt --check`)
- [ ] Linting passes (`cargo clippy`)
- [ ] No debug code or println! macros (except in logging)
- [ ] Test coverage is satisfactory
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

## Publishing Steps
1. Update version in `Cargo.toml`
2. Update `CHANGELOG.md`
3. Format code: `cargo fmt`
4. Run format check: `cargo fmt --check`
5. Run clippy: `cargo clippy -- -D warnings`
6. Run tests: `cargo test`
7. Run doc tests: `cargo test --doc`
8. Build in release mode: `cargo build --release`
9. Verify documentation: `cargo doc --no-deps`
10. Create a branch for the release following the pattern `release-<rust|node|python|>-<package-name>-v<version>`
11. Commit changes to the release branch and push to GitHub, with a commit message of `release <rust|node|python|> <package-name> v<version>`
12. Create a Pull Request to merge your changes to the main branch
13. Once the PR is approved and merged, tagging and publishing is done automatically by the CI pipeline

## Post-Publishing
- [ ] Verify package installation works: `cargo add lambda-otel-lite`
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
- Always use `cargo package` first to verify the package contents
- Test the package with different feature combinations
- Consider cross-platform compatibility
- Test with the minimum supported Rust version
- Consider running `cargo audit` for security vulnerabilities
- Use `cargo clippy` with all relevant feature combinations
- Remember to update any related documentation or examples in the main repository
- Consider testing on different architectures (x86_64, aarch64) 