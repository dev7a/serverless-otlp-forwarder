## livetrace v0.1.0 - 2025-05-10

This initial release of `livetrace` marks its first public version, bringing a powerful tool for local development and debugging of distributed traces in serverless environments that use the Serverless OTLP Forwarder architecture.

### Highlights of this Release:

*   **Shell Completion Support:** Boost your productivity with shell completion scripts for Bash, Zsh, Fish, and more. Generate them easily using the new `livetrace generate-completions <SHELL>` command.
*   **Comprehensive Documentation:** The `README.md` has been significantly enhanced with detailed usage examples, clear installation instructions (including for shell completions), and better alignment with the overall Serverless OTLP Forwarder architecture.
*   **Under-the-Hood Improvements:**
    *   Updated key dependencies like `chrono`, `nix`, and `indicatif` to their latest versions.
    *   Refactored internal code structure, particularly around AWS setup, and added module-level documentation for improved readability and maintainability.
*   **Metadata Updates:** Project description, keywords in `Cargo.toml`, and the license year have been updated to reflect the current state and focus of the tool.

This release focuses on providing a solid foundation with essential features and a great developer experience.

For a detailed list of all individual changes, bug fixes, and commits, please see the [CHANGELOG.md](./CHANGELOG.md).
