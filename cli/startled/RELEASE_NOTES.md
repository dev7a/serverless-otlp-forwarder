# Release Notes - startled v0.4.0

**Release Date:** 2025-06-21

This release enhances the `report` command with new customization options and improves the landing page UI.

## New Features

### Enhanced Report Customization
- **`--title` option**: Customize the title of your benchmark report landing page
- **`--description` option**: Add descriptive text to provide context about your benchmark results

## Improvements

### User Interface
- **Cleaner Landing Page**: Removed the duplicative items-grid section from the report landing page in favor of the existing sidebar navigation, resulting in a cleaner and less cluttered interface

### Documentation
- **Updated Examples**: CLI usage examples now demonstrate the new `--title` and `--description` options

## Example Usage

```bash
startled report \
  --input-dir ./benchmark_results \
  --output-dir ./reports \
  --title "Lambda Performance Analysis" \
  --description "Comprehensive comparison of different OpenTelemetry configurations across Node.js, Python, and Rust runtimes" \
  --screenshot dark
```

This release maintains full backward compatibility with existing workflows while providing new options for creating more professional and informative benchmark reports.