# Release Notes for startled v0.6.0

This release introduces version `0.8.0` of the `startled` CLI tool, adding significant new features, enhancements, and fixes to improve benchmarking, reporting, and visualization capabilities. The most notable changes include the addition of memory scaling analysis, enhanced screenshot functionality, centralized chart management, basic tracing support, and multiple refinements to user experience and error handling.

### New Features:
* **Memory Scaling Analysis**: Introduced comprehensive summary pages (`/all/summary/`) that provide performance metrics across various memory configurations (128mb, 256mb, 512mb, 1024mb), with interactive line charts for cross-configuration comparisons. [[1]](diffhunk://#diff-bd0cb949bb67fcfa38060059b5016cdb217ed459714094210b87496f0714b453R8-R50) [[2]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573R55) [[3]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573L255-R263) [[4]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573R355) [[5]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573L369-R374)
* **Enhanced Screenshot Functionality**: Overhauled screenshot system with dynamic height detection, theme-based backgrounds, improved timing logic, and support for all chart types, ensuring robust and high-quality PNG generation. [[1]](diffhunk://#diff-fe149fe84e623fbe578902b2a7b1de940feba1824d841deab416cd165b7e6752L17-R17) [[2]](diffhunk://#diff-fe149fe84e623fbe578902b2a7b1de940feba1824d841deab416cd165b7e6752L30-R112) [[3]](diffhunk://#diff-aff33bf4e337463eb1a6180a9b58b944752069b10f5ca6a932555ac84afe0573L67-R73)
* **Centralized Chart Management**: Added a `ChartManager` system to handle chart lifecycle management, eliminating duplication and memory leaks during theme switching.
* **Basic Tracing Support**: Implemented the `init_tracing()` function for lightweight debugging of the report command, controlled by the `TRACING_STDOUT` environment variable. [[1]](diffhunk://#diff-5fb52f72c3daaba5adfbdfaddaf0e2bc6b28ebbb5e4d9e8a4082eaf66b5d8886L224-R231) [[2]](diffhunk://#diff-03e2f68cbacf7c23a9129ff280a4f594c2e81ee146958c27319eddca01a43135R14-R34)

### Enhancements:
* **Chart Architecture Refactoring**: Improved JavaScript chart generation with better theming, tooltips, and a new `MemoryScalingCharts` module for multi-configuration visualizations.
* **Improved User Experience**: Enhanced chart interactions, responsive design, and DOM restoration for seamless theme switching.
* **Error Handling**: Added silent error handling for production use while maintaining debugging capabilities.

### Fixes:
* **Screenshot Timing Issue**: Resolved issues with unavailable CSS/JS files during screenshot generation by adjusting file copying order.
* **Theme Switching Stability**: Fixed chart disappearance and duplication problems during light/dark theme transitions.

### Miscellaneous:
* **Pricing Script**: Added a Python script (`pricing.py`) to fetch AWS Lambda compute pricing based on region and architecture.
* **Version Update**: Updated the package version from `0.7.0` to `0.8.0` in `Cargo.toml`.
* **Sidebar Update**: Modified sidebar structure in `_sidebar.html` to include links to memory scaling analysis pages.