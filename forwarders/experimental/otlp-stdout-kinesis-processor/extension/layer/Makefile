# Makefile for building the Rust Lambda Extension layer
# This builds the extension for ARM64 architecture and copies it to the artifacts directory
# The extension is used to capture and process Lambda telemetry events


build-ExtensionLayerARM64:
	@echo "Building Rust extension layer for ARM64"
	@cargo lambda build --release --extension --arm64 --bin otlp-stdout-kinesis-extension-layer-arm64
	@echo "Copying extension layer to artifacts directory"
	@mkdir -p "$(ARTIFACTS_DIR)/extensions"
	@cp "$(shell cargo metadata --format-version=1 | jq -r '.target_directory')/lambda/extensions/otlp-stdout-kinesis-extension-layer-arm64" "$(ARTIFACTS_DIR)/extensions/"
	@chmod +x "$(ARTIFACTS_DIR)/extensions/otlp-stdout-kinesis-extension-layer-arm64"
build-ExtensionLayerAMD64:
	@echo "Building Rust extension layer for AMD64"
	@cargo lambda build --release --extension --x86-64 --bin otlp-stdout-kinesis-extension-layer-amd64
	@echo "Copying extension layer to artifacts directory"
	@mkdir -p "$(ARTIFACTS_DIR)/extensions"
	@cp "$(shell cargo metadata --format-version=1 | jq -r '.target_directory')/lambda/extensions/otlp-stdout-kinesis-extension-layer-amd64" "$(ARTIFACTS_DIR)/extensions/"	
	@chmod +x "$(ARTIFACTS_DIR)/extensions/otlp-stdout-kinesis-extension-layer-amd64"