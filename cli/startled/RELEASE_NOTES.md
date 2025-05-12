# Release Notes - startled v0.3.1

**Release Date:** 2025-05-13

This release enhances the HTML report visualization with combined charts, improved styling, and better numerical precision.

## Key Highlights

- **Combined Chart Views:** Each chart page now shows both statistical aggregates (bar charts with AVG, P50, P95, P99) and time series data (line charts showing individual data points over time) on the same page for a more complete visualization.

- **Improved Visual Experience:**
  - Enhanced chart styling and layout for better readability
  - Updated color palette with more visually distinct colors
  - Better formatting of numerical values with consistent precision
  - Improved screenshot capabilities with larger viewport for capturing both chart types

- **User Experience Improvements:**
  - Added support for local browsing with appropriate link handling
  - Improved navigation between different chart types
  - Better handling of chart resizing for different screen sizes

## Technical Improvements

- Refactored chart rendering logic for better maintainability
- Fixed Clippy warnings related to function argument counts
- Improved screenshot reliability with additional wait time between rendering stages
