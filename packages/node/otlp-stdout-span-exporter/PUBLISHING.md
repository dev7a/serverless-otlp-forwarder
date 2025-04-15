# Publishing Checklist

Before publishing a new version of `@dev7a/otlp-stdout-span-exporter`, ensure all these items are checked:

## package.json Verification
- [ ] `version` is correctly incremented (following semver)
- [ ] `name` is correct
- [ ] `description` is clear and up-to-date
- [ ] `license` is specified
- [ ] `keywords` are defined and relevant
- [ ] `engines` requirements are appropriate
- [ ] `repository` information is complete and correct
- [ ] `homepage` URL is valid
- [ ] Dependencies are up-to-date and correct
- [ ] No extraneous dependencies
- [ ] Development dependencies are in `devDependencies`
- [ ] Peer dependencies are correctly specified
- [ ] Node.js version requirements are correctly specified

## Documentation
- [ ] `README.md` is up-to-date
- [ ] `CHANGELOG.md` is updated

## Code Quality
- [ ] All tests pass (`npm test`)
- [ ] Code is properly linted (`npm run lint`)
- [ ] TypeScript compilation works without errors (`tsc -p tsconfig.json`)
- [ ] No debug code or console.log statements (except in logging)
- [ ] Test coverage is satisfactory
- [ ] All public APIs have proper documentation
- [ ] All type definitions are present and correct
- [ ] All compiler warnings are addressed

## Git Checks
- [ ] Working on the correct branch
- [ ] All changes are committed
- [ ] No unnecessary files in git
- [ ] Git tags are ready to be created
- [ ] `.gitignore` is up-to-date

## Version Management
- [ ] Update version in `package.json` only
- [ ] Do NOT manually edit `version.ts` - it is auto-generated during build by the `generate:version` npm script

## Publishing Steps
1. Update version in `package.json` (this is the single source of truth for the version)
2. Update `CHANGELOG.md`
3. Run linting: `npm run lint`
4. Run tests: `npm test`
5. Build package: `npm run build` (this will automatically generate the version.ts file)
6. **Create and switch to a release branch** following the pattern `release/<rust|node|python|>/<package-name>-v<version>`
   - Example: `git checkout -b release/node/otlp-stdout-span-exporter-v0.13.0`
7. **Commit all changes to the release branch**
   - Example: `git add . && git commit -m "release: node/otlp-stdout-span-exporter v0.13.0"`
8. Push the release branch to GitHub
   - Example: `git push origin release/node/otlp-stdout-span-exporter-v0.13.0`
9. Tagging and publishing is done automatically by the CI pipeline

## Post-Publishing
- [ ] Verify package installation works: `npm install @dev7a/otlp-stdout-span-exporter`
- [ ] Verify documentation appears correctly on npm
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
- Node.js version compatibility issues

## Notes
- Always run `npm run build` before publishing (which automatically generates the `version.ts` file)
- Test the package with different Node.js versions
- Consider cross-platform compatibility
- Test with the minimum supported Node.js version
- Consider running security checks
- Remember to update any related documentation or examples in the main repository
- Ensure GZIP compression functionality is properly tested at all levels (0-9)
- Verify both simple and batch export modes work correctly
- Test with different header configurations and environment variables