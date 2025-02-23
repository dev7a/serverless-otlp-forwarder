# Publishing Checklist

Before publishing a new version of `@dev7a/lambda-otel-lite`, ensure all these items are checked:

## Package.json Verification
- [ ] `version` is correctly incremented (following semver)
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

## Publishing Steps
1. Update version in `package.json`
2. Update `CHANGELOG.md`
3. Format code: `npm run format`
4. Run format check: `npm run format:check`
5. Run linting: `npm run lint`
6. Run tests: `npm test`
7. Clean build: `npm run clean && npm run build`
8. Commit changes: `git commit -am "Release vX.Y.Z"`
9. Create git tag: `git tag vX.Y.Z`
10. Push changes: `git push && git push --tags`
11. Publish to npm: `npm publish`
12. Verify package on npm: https://www.npmjs.com/package/@dev7a/lambda-otel-lite

## Post-Publishing
- [ ] Verify package installation works: `npm install @dev7a/lambda-otel-lite`
- [ ] Verify documentation appears correctly on npm
- [ ] Verify all package files are included
- [ ] Test the package in a new project
- [ ] Update any dependent packages

## Common Issues to Check
- Missing files in the published package
- Incorrect peer dependencies
- Missing type definitions
- Broken links in documentation
- Incorrect version numbers
- Missing changelog entries
- Unintended breaking changes

## Notes
- Always use `npm publish --dry-run` first to verify the package contents
- Consider using `npm pack` to inspect the exact files that will be published
- Test the package in a clean environment before publishing
- Consider the impact on dependent packages 