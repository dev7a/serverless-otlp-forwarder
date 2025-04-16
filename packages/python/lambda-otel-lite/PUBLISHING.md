# Publishing Checklist

Before publishing a new version of `lambda_otel_lite`, ensure all these items are checked:

## pyproject.toml Verification
- [ ] `version` is correctly incremented (following semver)
- [ ] Version number matches latest version in CHANGELOG.md
- [ ] `description` is clear and up-to-date
- [ ] `license` is specified correctly
- [ ] `keywords` are defined and relevant
- [ ] `classifiers` are accurate and up-to-date
- [ ] `requires-python` is set correctly
- [ ] `repository` and `homepage` URLs are valid
- [ ] Dependencies are up-to-date and correct
- [ ] No extraneous dependencies in `dependencies`
- [ ] Development tools are in `optional-dependencies.dev`
- [ ] Build system configuration is correct

## Documentation
- [ ] `README.md` is up-to-date
- [ ] Version number in documentation matches pyproject.toml
- [ ] All examples in documentation work with current version
- [ ] API documentation is complete
- [ ] Breaking changes are clearly documented
- [ ] `CHANGELOG.md` is updated
- [ ] Environment variables are documented
- [ ] All supported event types are documented

## Code Quality
- [ ] All tests pass (`pytest`)
- [ ] Test coverage is satisfactory (`pytest --cov`)
- [ ] Type checking passes (`mypy`)
- [ ] Linting passes (`ruff check --isolated src/lambda_otel_lite tests` and `ruff format --check --isolated src/lambda_otel_lite tests`)
- [ ] No debug code or print statements (except in logger)
- [ ] All public APIs are properly typed
- [ ] All docstrings are complete and up-to-date

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
5. The `.gitignore` file in the package directory excludes the generated `version.py` from version control

This approach ensures that:
- We have a single source of truth for version information
- Version information is always correct and in sync
- The version is accessible via `from lambda_otel_lite import __version__`

## Publishing Steps
1. Update version in `pyproject.toml` (this is the single source of truth for the version)
2. Update `CHANGELOG.md`
3. Run quality checks:
   ```bash
   pytest
   mypy src/lambda_otel_lite
   ruff check --isolated src/lambda_otel_lite tests
   ruff format --check --isolated src/lambda_otel_lite tests
   ```
4. Clean build:
   ```bash
   rm -rf dist/ build/ *.egg-info/
   python -m build
   ```
5. Test the build:
   ```bash
   python -m venv test_venv
   source test_venv/bin/activate
   pip install dist/*.whl
   pytest  # Run tests with installed package
   deactivate
   rm -rf test_venv
   ```
6. Create a branch for the release following the pattern `release/<rust|node|python|>/<package-name>-v<version>`
7. Commit changes to the release branch and push to GitHub, with a commit message of `release: <rust|node|python|>/<package-name> v<version>`
8. Create a pull request from the release branch to main
9. Once the PR is approved and merged, tagging and publishing is done automatically by the CI pipeline

## Post-Publishing
- [ ] Verify package installation works: `pip install lambda_otel_lite`
- [ ] Verify documentation appears correctly on PyPI
- [ ] Verify all package files are included
- [ ] Test the package in a new project
- [ ] Update any dependent packages
- [ ] Verify examples work with the published version

## Common Issues to Check
- Missing files in the published package
- Incorrect Python version requirements
- Missing type hints or stub files
- Broken links in documentation
- Incorrect version numbers
- Missing changelog entries
- Unintended breaking changes
- Incomplete or incorrect package metadata

## Notes
- Always use `python -m build` to build both wheel and sdist (which automatically generates the `version.py` file)
- Use `twine check dist/*` to verify package metadata before uploading
- Test the package in a clean virtual environment before publishing
- Consider the impact on dependent packages
- Make sure all required files are included in the package (check MANIFEST.in if needed)
- Verify that the package works with the minimum supported Python version
- Consider testing on multiple operating systems if possible
- Remember to update any related documentation or examples in the main repository 