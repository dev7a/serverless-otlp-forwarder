# Release Notes - startled v0.4.1

**Release Date:** 2025-06-21

This is a **critical bug fix release** that resolves an issue preventing distributed binaries from functioning properly.

## Bug Fix

### JavaScript File Embedding Issue
- **Fixed**: Resolved "Failed to copy default lib.js" error that occurred when using startled binaries downloaded from GitHub releases
- **Root Cause**: The JavaScript file copying logic was using `env!("CARGO_MANIFEST_DIR")` which only exists during compilation, not in distributed binaries
- **Solution**: Changed to use `include_str!()` to properly embed the JavaScript file at compile time, consistent with how CSS files are handled

## Impact

This fix ensures that the `report` command works correctly in all deployment scenarios:
- ✅ **Development**: Works when building from source
- ✅ **CI/CD**: Works when downloaded from GitHub releases
- ✅ **Distribution**: Works for all binary distribution methods

## Migration

No action required - this is a drop-in replacement for v0.4.0. All existing functionality remains unchanged.