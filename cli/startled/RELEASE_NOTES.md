# Release Notes for startled v0.5.1

## 🎯 New Features

### Custom File Extensions for Reports

The `report` command now supports a `--suffix` option that allows you to generate reports with custom file extensions. This enhancement enables generating reports in different formats when combined with custom templates:

- **Default behavior unchanged**: Reports continue to generate as `.html` files by default
- **Flexible output formats**: Generate `.md`, `.txt`, `.json`, or any other format
- **Template integration**: Works seamlessly with the existing `--template-dir` feature

### Usage Examples

```bash
# Generate standard HTML reports (default)
startled report -d input/ -o output/

# Generate Markdown reports with custom templates
startled report -d input/ -o output/ --suffix md --template-dir ./markdown-templates

# Generate plain text reports
startled report -d input/ -o output/ --suffix txt --template-dir ./text-templates
```

This feature is particularly useful for:
- Generating Markdown documentation from benchmark results
- Creating plain text reports for CLI tools
- Producing custom formats for integration with other systems

## 📦 Installation

```bash
cargo install startled --version 0.5.1
```

## 📝 What's Changed

- Added `--suffix` CLI option to the `report` command
- Updated file generation logic to use `index.{suffix}` instead of hardcoded `index.html`
- Enhanced documentation with examples of generating different file formats

## 🙏 Acknowledgments

Thank you to all contributors and users who provide feedback to make startled better!

# Release Notes - startled v0.5.0

**Release Date:** 2025-06-21

This release introduces **AWS-Documentation-Based Metric Descriptions**, a major enhancement that transforms the startled reports from raw performance data into comprehensive, educational Lambda performance analysis tools.

## 🎯 Major New Feature

### AWS-Documentation-Based Metric Descriptions

Every metric chart now includes detailed, expert-level descriptions that explain:

- **What each metric represents** in AWS Lambda's execution model
- **Official AWS CloudWatch metric equivalents** (Duration, PostRuntimeExtensionsDuration, MaxMemoryUsed)
- **Platform-level metrics** from AWS's internal instrumentation (platform.runtimeDone)
- **Performance implications** and optimization insights
- **Measurement context** (cold starts vs warm starts, initialization phases)

#### Coverage Includes:

**Cold Start Metrics:**
- Init Duration, Server Duration, Extension Overhead
- Total Cold Start Duration, Response Latency/Duration  
- Runtime Overhead, Runtime Done Duration

**Warm Start Metrics:**
- Client Duration, Server Duration, Extension Overhead
- Response Latency/Duration, Runtime Overhead, Runtime Done Duration

**Resource Metrics:**
- Memory Usage, Produced Bytes

## 🎨 Visual Enhancements

### Improved User Experience
- **Dedicated metric description sections** with professional styling
- **Enhanced color contrast** in dark theme for better readability  
- **Improved background colors** in light theme
- **Streamlined readme content styling**

## 📚 Educational Value

This release transforms startled from a charting tool into a **Lambda performance education platform**. Users now understand:

- How AWS Lambda's execution environment lifecycle affects performance
- The relationship between different timing metrics
- Which metrics correspond to AWS CloudWatch billing and monitoring
- How extensions impact Lambda performance across cold/warm starts
- Platform-level insights from AWS's internal telemetry

## 🔧 Implementation Details

- **Research-based descriptions** derived from official AWS Lambda documentation
- **Consistent styling** with dedicated CSS classes for metric descriptions
- **Template integration** that automatically displays relevant descriptions
- **Comprehensive test coverage** ensuring description accuracy

## 🚀 Usage Example

```bash
startled report \
    --dir=results \
    --output=./reports \
    --title "Lambda Performance Analysis" \
    --description "Deep dive into runtime performance characteristics"
```

Each generated chart now includes contextualized explanations that help users:
- Identify performance bottlenecks
- Understand extension overhead impact  
- Correlate metrics with AWS CloudWatch data
- Make informed optimization decisions

## 📈 Impact

This enhancement addresses a key gap in Lambda performance tooling - transforming raw metrics into actionable insights through expert-level explanations based on official AWS documentation.

---

## Compatibility

- ✅ **Backwards Compatible**: All existing functionality preserved
- ✅ **No Breaking Changes**: Existing CLI usage remains unchanged  
- ✅ **Template Compatibility**: Custom templates automatically benefit from new descriptions