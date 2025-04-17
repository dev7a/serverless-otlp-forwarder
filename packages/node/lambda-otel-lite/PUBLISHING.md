# Publishing Checklist

Before publishing a new version of `@dev7a/lambda-otel-lite`, ensure all these items are checked:

## Package.json Verification
- [ ] `version` is correctly incremented (following semver)
- [ ] `name` is correct
- [ ] `description` is clear and up-to-date
- [ ] `license` is specified
- [ ] `keywords` are defined and relevant
- [ ] `repository` information is complete and correct
- [ ] `homepage` URL is valid
- [ ] `bugs` URL is specified
- [ ] `main` and `types` fields are correct
- [ ] `exports` map is complete and correct
- [ ] `files` list includes all necessary files
- [ ] Dependencies are up-to-date and correct
- [ ] No extraneous dependencies in `dependencies`
- [ ] Development tools are in `devDependencies`

## Documentation
- [ ] `README.md` is up-to-date
- [ ] Version number in documentation matches package.json
- [ ] All examples in documentation work
- [ ] API documentation is complete
- [ ] Breaking changes are clearly documented
- [ ] `CHANGELOG.md` is updated

## Code Quality
- [ ] All tests pass (`npm test`)
- [ ] Code is properly formatted (`npm run format`)
- [ ] Format check passes (`npm run format:check`)
- [ ] Linting passes (`npm run lint`)
- [ ] TypeScript compilation succeeds (`npm run build`)
- [ ] No debug code or console.logs (except in logger)
- [ ] Code coverage is satisfactory
- [ ] All exports are properly typed

## Git Checks
- [ ] Working on the correct branch
- [ ] All changes are committed
- [ ] No unnecessary files in git
- [ ] Git tags are ready to be created

## Version Management
- [ ] Update version in `package.json` only
- [ ] Do NOT manually edit `version.ts` - it is auto-generated during build by the `generate:version` npm script

## Publishing Steps
1. Update version in `package.json` (this is the single source of truth for the version)
2. Update `CHANGELOG.md`
3. Format code: `npm run format`
4. Run format check: `npm run format:check`
5. Run linting: `npm run lint`
6. Run tests: `npm test`
7. Build package: `npm run build` (this will automatically generate the version.ts file)
8. Create a branch for the release following the pattern `release/<rust|node|python|>/<package-name>-v<version>`
9. Commit changes to the release branch and push to GitHub, with a commit message of `release: <rust|node|python|>/<package-name> v<version>`
10. Create a pull request from the release branch to main
11. Once the PR is approved and merged, tagging and publishing is done automatically by the CI pipeline

## Post-Publishing
- [ ] Verify package installation works: `npm install @dev7a/lambda-otel-lite`
- [ ] Verify documentation appears correctly on npm
- [ ] Verify all package files are included
- [ ] Test the package in a new project
- [ ] Update any dependent packages
- [ ] Verify examples run correctly

## Common Issues to Check
- Missing files in the published package
- Incorrect peer dependencies
- Missing type definitions
- Broken links in documentation
- Incorrect version numbers
- Missing changelog entries
- Unintended breaking changes

## Notes
- Always run `npm run build` before publishing (which automatically generates the `version.ts` file)
- Test the package with different Node.js versions
- Consider cross-platform compatibility
- Test with the minimum supported Node.js version
- Consider running security checks
- Remember to update any related documentation or examples in the main repository 