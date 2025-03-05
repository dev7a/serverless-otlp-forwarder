# Publishing Checklist

Before publishing a new version of `otlp-stdout-span-exporter`, ensure all these items are checked:

## pyproject.toml Verification
- [ ] `version` is correctly incremented (following semver)
- [ ] `name` is correct
- [ ] `description` is clear and up-to-date
- [ ] `license` is specified
- [ ] `keywords` are defined and relevant
- [ ] `classifiers` are appropriate
- [ ] `repository` information is complete and correct
- [ ] `homepage` URL is valid
- [ ] Dependencies are up-to-date and correct
- [ ] No extraneous dependencies
- [ ] Development dependencies are in `dev` optional dependencies
- [ ] Python version requirements are correctly specified

## Documentation
- [ ] `README.md` is up-to-date
- [ ] `CHANGELOG.md` is updated

## Code Quality
- [ ] All tests pass (`pytest`)
- [ ] Code is properly formatted (`ruff format`)
- [ ] Linting passes (`ruff check`)
- [ ] Type checking passes (`mypy`)
- [ ] No debug code or print statements (except in logging)
- [ ] Test coverage is satisfactory (`pytest --cov`)
- [ ] All public APIs have proper documentation
- [ ] All type hints are present and correct
- [ ] All compiler warnings are addressed

## Git Checks
- [ ] Working on the correct branch
- [ ] All changes are committed
- [ ] No unnecessary files in git
- [ ] Git tags are ready to be created
- [ ] `.gitignore` is up-to-date

## Version Management
- [ ] Update version in `pyproject.toml` only
- [ ] Do NOT manually edit `version.py` - it is auto-generated during build

## How Version Management Works

The package uses Hatch's built-in version hook to automatically generate the `version.py` file during build time:

1. The single source of truth for the version is in `pyproject.toml`
2. During the build process, Hatch reads the version from `pyproject.toml`
3. Hatch generates the `version.py` file with both `VERSION` and `__version__` variables
4. The `artifacts` configuration in `pyproject.toml` ensures this file is included in both sdist and wheel distributions
5. The `.gitignore` file excludes the generated `version.py` from version control

This approach ensures that:
- We have a single source of truth for version information
- Version information is always correct and in sync
- The version is accessible via `from otlp_stdout_span_exporter import VERSION`

## Publishing Steps
1. Update version in `pyproject.toml` (this is the single source of truth for the version)
2. Update `CHANGELOG.md`
3. Format code: `ruff format --isolated src/otlp_stdout_span_exporter tests`
4. Run linting: `ruff check --isolated src/otlp_stdout_span_exporter tests`
5. Run type checking: `mypy src/otlp_stdout_span_exporter`
6. Run tests: `pytest`
7. Run coverage: `pytest --cov`
8. Build package: `python -m build` (this will automatically generate the version.py file)
9. Create a branch for the release following the pattern `release-<rust|node|python|>-<package-name>-v<version>`
10. Tagging and publishing is done automatically by the CI pipeline

## Post-Publishing
- [ ] Verify package installation works: `pip install otlp-stdout-span-exporter`
- [ ] Verify documentation appears correctly on PyPI
- [ ] Test the package in a new project
- [ ] Update any dependent packages
- [ ] Verify examples run correctly

## Common Issues to Check
- Missing files in the published package
- Missing or incorrect documentation
- Broken links in documentation
- Incorrect version numbers
- Missing changelog entries
- Unintended breaking changes
- Incomplete package metadata
- Platform-specific issues
- Python version compatibility issues

## Notes
- Always use `python -m build` to build the package (which automatically generates the `version.py` file)
- Test the package with different Python versions
- Consider cross-platform compatibility
- Test with the minimum supported Python version
- Consider running security checks
- Remember to update any related documentation or examples in the main repository
- Ensure GZIP compression functionality is properly tested at all levels (0-9)
- Verify both simple and batch export modes work correctly
- Test with different values of `OTLP_STDOUT_SPAN_EXPORTER_COMPRESSION_LEVEL` environment variable