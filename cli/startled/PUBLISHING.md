# Publishing a New Release of `startled`

This document outlines the steps to publish a new version of the `startled` CLI crate.

## Release Process

1.  **Ensure Main Branch is Up-to-Date**:
    *   Switch to your main development branch (e.g., `main` or `develop`).
        ```bash
        git checkout main
        git pull origin main
        ```
    *   Ensure your local main branch is synchronized with the remote.

2.  **Create Release Branch**:
    *   Create a new release branch from the up-to-date main branch:
        ```bash
        git checkout -b release/cli/startled-v<VERSION_PLACEHOLDER> 
        ```
        (e.g., `release/cli/startled-v0.2.0_candidate` - use a placeholder initially if exact version is TBD based on diff).

3.  **Review Changes & Determine Version**:
    *   Run `git diff main...HEAD cli/startled | cat` (or compare against the last release tag) to review all changes specific to the `cli/startled` directory for this release.
    *   Based on the `git diff`, decide if the release is a `patch` or `minor` update according to Semantic Versioning (SemVer) for 0.x releases:
        *   **Patch (0.x.Y -> 0.x.Z, where Z > Y)**: For backwards-compatible bug fixes.
        *   **Minor (0.X.y -> 0.Y.z, where Y > X)**: For new backwards-compatible functionality.
    *   Finalize the `<VERSION>` number (e.g., `0.2.0`).
    *   If the branch name used a placeholder, rename it now if desired (optional):
        ```bash
        git branch -m release/cli/startled-v<VERSION>
        ```

4.  **Update `Cargo.toml`**:
    *   Edit `cli/startled/Cargo.toml` and set the `version` field to the new `<VERSION>`.

5.  **Run Quality Checks**:
    *   **5.1. Run Tests**:
        ```bash
        cargo test -p startled
        ```
        Ensure all tests pass. Correct any failures.
    *   **5.2. Run Clippy (Linter)**:
        ```bash
        cargo clippy --all-targets -- -D warnings 
        ```
        Address all warnings and errors.

6.  **Update Documentation Files**:
    *   **6.1. Update `CHANGELOG.md`**:
        *   Add a new entry at the top of `cli/startled/CHANGELOG.md` for `[<VERSION>] - <YYYY-MM-DD>`.
        *   Summarize changes based on the `git diff` (Added, Changed, Fixed, Removed).
    *   **6.2. Prepare `RELEASE_NOTES.md`**:
        *   Update or create `cli/startled/RELEASE_NOTES.md` for `v<VERSION>`.
    *   **6.3. Update `README.md`**:
        *   Review `cli/startled/README.md` and update it to reflect any new features, CLI options, changes in behavior, or other important information introduced in this release.
    *   **6.4. Update `PUBLISHING.md` (this file)** if any part of the release process itself has changed.

7.  **Final Validation with Dry Run**:
    *   Navigate to the crate directory: `cd cli/startled` (if not already there).
    *   Perform a `cargo publish --dry-run --allow-dirty`. This checks for common packaging issues without actually publishing.
        ```bash
        cargo publish --dry-run
        ```
    *   Address any errors or warnings from the dry run.

8.  **Commit Release Preparation**:
    *   Stage all changes related to the release (Cargo.toml, CHANGELOG.md, RELEASE_NOTES.md, PUBLISHING.md if updated, and any code fixes from quality checks):
        ```bash
        git add cli/startled/
        ```
    *   Verify staged files:
        ```bash
        git status
        ```
    *   Commit the changes:
        ```bash
        git commit -m "release: cli/startled v<VERSION>"
        ```

9.  **Push Release Branch**:
    *   Push the release preparation branch to the remote repository:
        ```bash
        git push origin release/cli/startled-v<VERSION>
        ```

10. **Open a Pull Request (PR)**:
    *   Open a new PR from `release/cli/startled-v<VERSION>` to the main development branch.
    *   Ensure the PR description includes or links to the release notes.
    *   Wait for CI checks to pass and for code review.

11. **Merge the PR**:
    *   Once approved and all checks pass, merge the PR into the main development branch (e.g., using a squash merge if preferred, to keep the main branch history clean with a single commit for the release prep).

12. **Automated Post-Merge Steps (CI/CD)**:
    *   Upon merging the release PR (or a push to the main branch with the correct version commit), the CI/CD pipeline configured in `.github/workflows/publish-startled.yml` should automatically:
        *   Create a Git tag (e.g., `cli/startled/v<VERSION>`).
        *   Publish the `startled` crate (version from `Cargo.toml`) to Crates.io.
        *   Create a corresponding GitHub Release, potentially using `RELEASE_NOTES.md`.

---
This checklist should be followed for each new release to ensure consistency and quality. 