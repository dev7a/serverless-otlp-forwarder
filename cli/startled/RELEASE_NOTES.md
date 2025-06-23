# Release Notes for startled v0.6.0

## 🎯 Major Enhancements

### Consistent Directory Naming & Navigation Overhaul

This release introduces a **comprehensive reorganization** of the report interface for improved consistency and usability. All warm start metrics now follow a unified `warm-start-` naming convention, and the navigation has been streamlined for better logical organization.

#### Directory Naming Consistency
All warm start charts now use consistent `warm-start-` prefixes:
- `client-duration/` → `warm-start-client-duration/`
- `server-duration/` → `warm-start-server-duration/`
- `extension-overhead/` → `warm-start-extension-overhead/`
- `memory-usage/` → `warm-start-memory-usage/`
- `produced-bytes/` → `warm-start-produced-bytes/`

### Complete Cold Start Resource Coverage

Added **missing cold start variants** for resource metrics:
- **Cold Start Memory Usage**: Track memory consumption during Lambda initialization
- **Cold Start Produced Bytes**: Monitor response payload sizes during cold starts

These metrics were previously only available for warm starts, creating an incomplete picture of resource usage patterns.

### Reorganized Navigation Layout

**Eliminated the redundant "Resources" section** and logically integrated memory and produced bytes metrics directly into their respective execution contexts:

- **Cold Start Section**: Now includes memory usage and produced bytes alongside platform metrics
- **Warm Start Section**: Similarly organized with all relevant metrics in one place
- **Improved Logical Flow**: "Total Cold Start Duration" moved to the end for better ordering

### Optimized CSS Layout

**Enhanced navigation styling** to accommodate the complete metric set:
- **2-Row Desktop Layout**: Clean organization supporting 14+ navigation buttons
- **Responsive Design**: Optimized button sizing (`calc(100% / 14)`, `min-width: 5.5rem`)
- **Better Space Utilization**: Efficient use of horizontal space

## 🔧 Complete Platform Metrics Coverage

Added the **remaining warm start platform metrics** for comprehensive performance analysis:
- **Response Latency**: Platform-level response timing
- **Response Duration**: Response transmission time  
- **Runtime Overhead**: Lambda runtime processing overhead
- **Runtime Done Duration**: Complete runtime cycle measurement

## 📚 Enhanced Metric Descriptions

**Updated and expanded** AWS-documentation-based descriptions to reflect cold/warm start distinctions for all resource metrics, providing clearer guidance on:
- When and why memory usage differs between cold and warm starts
- How response payload sizes impact performance in different execution contexts
- The relationship between resource consumption and execution phases

## 🎨 User Experience Improvements

### Navigation Clarity
- **Logical Grouping**: Metrics organized by execution context (cold start vs warm start)
- **Consistent Naming**: All external directory names follow kebab-case conventions
- **Better Flow**: Metrics ordered logically within each section

### Complete Data Coverage
- **No Missing Data**: Both cold and warm start scenarios now have complete resource metric coverage
- **Balanced Analysis**: Users can now compare resource usage patterns across all execution types

## 💻 Usage Examples

```bash
# Generate reports with the new improved navigation
startled report \
    --dir=benchmark-results \
    --output=./enhanced-reports \
    --title "Comprehensive Lambda Performance Analysis" \
    --description "Complete cold/warm start performance comparison with resource metrics"
```

Reports now provide:
- **Comprehensive Cold Start Analysis**: Including resource consumption during initialization
- **Complete Warm Start Coverage**: Full platform and resource metrics
- **Improved Navigation**: Intuitive organization by execution context
- **Consistent Interface**: Uniform naming and layout across all charts

## 🔄 Migration Notes

### Breaking Changes
- **Directory Structure**: Warm start chart directories have new names (affects bookmarks/links)
- **Navigation Layout**: "Resources" section removed - metrics moved to appropriate execution contexts

### Backward Compatibility
- **CLI Interface**: No changes to command-line usage
- **Data Format**: JSON output format unchanged
- **Template System**: Custom templates may need updates for new navigation structure

## 🏆 Impact

This release transforms startled into a more **professional and intuitive** Lambda performance analysis tool:

- **Consistency**: Unified naming conventions across all interfaces
- **Completeness**: No missing data for any execution scenario  
- **Usability**: Logical organization that matches how developers think about Lambda performance
- **Scalability**: Navigation design that accommodates future metric additions

The reorganized interface makes it significantly easier to:
- Compare cold vs warm start resource usage
- Navigate between related metrics
- Understand the complete performance picture
- Generate professional reports for stakeholders

---

## 📦 Installation

```bash
cargo install startled --version 0.6.0
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