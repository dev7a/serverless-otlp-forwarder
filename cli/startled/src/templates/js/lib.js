/**
 * Consolidated JavaScript library for the startled CLI report generator
 * 
 * This file contains all the JavaScript functionality needed for the benchmark reports:
 * 1. Base functionality for theming, UI behavior, and chart initialization
 * 2. Bar chart generation for metrics like cold start, warm start, and memory usage
 * 3. Line chart generation for time-series data like client duration over time
 * 
 * Users can customize this file to change the appearance and behavior of the reports.
 * When providing a custom template directory with --template-dir, place a modified
 * version of this file at: <your-template-dir>/js/lib.js
 */

// Default color palette for charts (can be customized)
window.DEFAULT_COLOR_PALETTE = [
    "#3fb1e3",
    "#6be6c1",
    "#a0a7e6",
    "#c4ebad",
    "#96dee8",
    "#e396d0",
    "#e3b396",
    "#966be3",
    "#626c91",
    "#96e3a7"
];


// ===============================
// Core UI and Setup Functionality
// ===============================

// Theme and chart globals
let barChart;
let lineChart;
const root = document.documentElement;

/**
 * Sets the theme for the entire report and initializes/reinitializes the charts
 * @param {string} theme - The theme name ('light' or 'dark')
 * @param {boolean} savePreference - Whether to save the preference to localStorage
 */
function setTheme(theme, savePreference = false) {
    root.setAttribute('data-theme', theme);
    
    // Only save to localStorage when explicitly requested
    if (savePreference) {
        localStorage.setItem('theme', theme);
    }
    
    // Update icons if present
    const darkIcon = document.querySelector('.dark-icon');
    const lightIcon = document.querySelector('.light-icon');
    if (darkIcon && lightIcon) {
        if (theme === 'dark') {
            darkIcon.style.display = 'block';
            lightIcon.style.display = 'none';
        } else {
            darkIcon.style.display = 'none';
            lightIcon.style.display = 'block';
        }
    }
    // Dispose existing charts
    if (barChart) {
        barChart.dispose();
    }
    
    if (lineChart) {
        lineChart.dispose();
    }
    
    SummaryCharts.cleanup();
    
    // Initialize charts based on data type
    initializeCharts(theme);
}

/**
 * Initializes the charts based on the available data
 * @param {string} theme - The theme to use ('light' or 'dark')
 */
function initializeCharts(theme) {
    if (!window.currentChartSpecificData) {
        console.error("No chart data available. Cannot initialize charts.");
        return;
    }

    // Get DOM elements for charts - handle summary page transformation
    let barChartDom = document.getElementById('chart_bar');
    const lineChartDom = document.getElementById('chart_time');
    const summaryGridDom = document.getElementById('summary-charts-grid');
    
    // If chart_bar doesn't exist but summary-charts-grid does, we're on summary page
    if (!barChartDom && summaryGridDom) {
        // For summary page, we don't need the bar chart DOM
        // Summary charts are handled separately
    } else if (!barChartDom) {
        console.error("Bar chart DOM element with id 'chart_bar' not found.");
        return;
    }

    // Handle different chart data types
    if (window.currentChartSpecificData.Combined && barChartDom) {
        // Combined charts - initialize both bar and line charts
        const combinedData = window.currentChartSpecificData.Combined;
        
        // Initialize bar chart
        barChart = echarts.init(barChartDom, theme);
        let barOptions = BarCharts.generateOptions({ Bar: combinedData.bar });
        if (barOptions && typeof barOptions.color === 'undefined' && window.DEFAULT_COLOR_PALETTE) {
            barOptions.color = window.DEFAULT_COLOR_PALETTE;
        }
        barChart.setOption(barOptions);
        
        // Initialize line chart if DOM element exists
        if (lineChartDom) {
            lineChart = echarts.init(lineChartDom, theme);
            let lineOptions = LineCharts.generateOptions({ Line: combinedData.line });
            if (lineOptions && typeof lineOptions.color === 'undefined' && window.DEFAULT_COLOR_PALETTE) {
                lineOptions.color = window.DEFAULT_COLOR_PALETTE;
            }
            lineChart.setOption(lineOptions);
        }
    } else if (window.currentChartSpecificData.Bar && barChartDom) {
        // Bar chart only
        barChart = echarts.init(barChartDom, theme);
        let options = BarCharts.generateOptions(window.currentChartSpecificData);
        if (options && typeof options.color === 'undefined' && window.DEFAULT_COLOR_PALETTE) {
            options.color = window.DEFAULT_COLOR_PALETTE;
        }
        barChart.setOption(options);
    } else if (window.currentChartSpecificData.Line && lineChartDom) {
        // Line chart only
        lineChart = echarts.init(lineChartDom, theme);
        let options = LineCharts.generateOptions(window.currentChartSpecificData);
        if (options && typeof options.color === 'undefined' && window.DEFAULT_COLOR_PALETTE) {
            options.color = window.DEFAULT_COLOR_PALETTE;
        }
        lineChart.setOption(options);
    } else if (window.currentChartSpecificData.Summary) {
        // Summary page - multiple charts (legacy format)
        SummaryCharts.initialize(window.currentChartSpecificData.Summary, theme);
    } else if (window.chartType === "summary" && window.currentChartSpecificData.metrics) {
        // Summary page with metrics data format
        // Dispose existing charts
        SummaryCharts.cleanup();
        
        // Transform layout
        SummaryCharts.createSummaryLayout();
        
        // Create individual charts
        window.currentChartSpecificData.metrics.forEach((metric, index) => {
            SummaryCharts.createMetricChart(metric, index, theme);
        });
    } else {
        console.error('Unknown chart data format:', window.currentChartSpecificData);
    }
}

/**
 * Prepares the page for taking screenshots
 * @param {string} theme - The theme to use for the screenshot
 */
function prepareScreenshot(theme) {
    setTheme(theme, false);
    // Hide sidebar and adjust layout for screenshots
    const sidebar = document.querySelector('.sidebar');
    const mainContent = document.querySelector('.main-content');
    const sidebarToggle = document.querySelector('.sidebar-toggle');
    const navBar = document.querySelector('div.header nav');
    if (sidebar) sidebar.style.display = 'none';
    if (mainContent) {
        mainContent.style.marginLeft = '0';
        mainContent.style.width = '100%';
        mainContent.style.maxWidth = '100%';
    }
    if (sidebarToggle) sidebarToggle.style.display = 'none';
    if (navBar) navBar.style.display = 'none';
    // Resize charts after DOM updates
    setTimeout(() => {
        if (barChart) {
            barChart.resize();
        }
        if (lineChart) {
            lineChart.resize();
        }
    }, 200);
}

// ============================
// Bar Chart Generator Module
// using Apache echarts.js (https://echarts.apache.org/en/index.html)
// ============================

/**
 * Module for generating bar chart options
 * Used for visualizing metrics like cold start times, memory usage, etc.
 */
const BarCharts = {
    /**
     * Generates ECharts options for bar charts
     * @param {Object} chartSpecificData - The chart data from the server
     * @returns {Object} ECharts options object
     */
    generateOptions: function(chartSpecificData) {
        if (!chartSpecificData || !chartSpecificData.Bar) {
            console.error("Invalid data format for bar chart generator:", chartSpecificData);
            return {}; // Return empty options on error
        }
        const data = chartSpecificData.Bar;

        const echartsSeries = data.series.map(s => ({
            name: s.name,
            type: 'bar',
            label: {
                show: true,
                position: 'right',
                formatter: `{c} ${data.unit}`
            },
            data: s.values.map((value, index) => ({
                value: value,
                name: data.y_axis_categories[index] // Assumes values align with categories
            }))
        }));

        const options = {
            backgroundColor: "transparent",
            title: {
                text: data.title.toUpperCase(),
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" }
            },
            tooltip: { 
                trigger: "axis", 
                axisPointer: { type: "shadow" } 
            },
            // Color palette will be applied by lib.js 
            legend: {
                orient: "horizontal",
                bottom: 5,
                type: "scroll"
            },
            grid: [{
                left: "10%", top: "15%", right: "15%", bottom: "10%",
                containLabel: true
            }],
            xAxis: [{
                type: "value",
                name: `${data.unit === "MB" ? "Memory" : "Duration"} (${data.unit})`,
                nameLocation: "middle",
                nameGap: 30,
                axisLabel: { formatter: `{value} ${data.unit}` },
                minInterval: 1
            }],
            yAxis: [{
                type: "category",
                inverse: true,
                data: data.y_axis_categories
            }],
            series: echartsSeries,
            toolbox: {
                feature: { restore: {}, saveAsImage: {} },
                right: "20px"
            },
            // Base responsive design (can be customized further in templates)
            media: [
                {
                    query: { maxWidth: 768 },
                    option: {
                        legend: {
                            top: "auto",
                            bottom: 5,
                            orient: "horizontal"
                        },
                        grid: [{
                            left: "5%",
                            right: "8%",
                            top: "10%",
                            bottom: "18%" 
                        }],
                        xAxis: [{
                            nameGap: 20,
                            axisLabel: { fontSize: 10 },
                            nameTextStyle: { fontSize: 11 }
                        }],
                        yAxis: [{
                            axisLabel: { fontSize: 10 },
                            nameTextStyle: { fontSize: 11 }
                        }]
                    }
                }
            ]
        };

        return options;
    }
};

// ============================
// Line Chart Generator Module
// ============================

/**
 * Module for generating line/scatter chart options
 * Used for time-series data like client duration over time
 */
const LineCharts = {
    /**
     * Generates ECharts options for line/scatter charts
     * @param {Object} chartSpecificData - The chart data from the server
     * @returns {Object} ECharts options object
     */
    generateOptions: function(chartSpecificData) {
        if (!chartSpecificData || !chartSpecificData.Line) {
            console.error("Invalid data format for line chart generator:", chartSpecificData);
            return {}; // Return empty options on error
        }
        const data = chartSpecificData.Line;

        // Determine y-axis max based on P90 of all points
        const allYValues = data.series.flatMap(s => s.points.map(p => p.y));
        let yMax = 1000; // Default max
        if (allYValues.length > 0) {
            // Simple P90 calculation (sort and pick) - might need a more robust library for large datasets
            allYValues.sort((a, b) => a - b);
            const p90Index = Math.floor(allYValues.length * 0.9);
            const p90 = allYValues[p90Index];
            yMax = p90 * 1.2; // Add some headroom
        }

        // Transform series data for ECharts
        const echartsSeries = data.series.map(s => {
            const seriesPoints = s.points.map(p => ({
                value: [p.x, p.y] // ECharts scatter data format [x, y]
            }));

            const markLineData = [];
            if (s.mean !== null && s.mean !== undefined) {
                 const lastPointX = s.points.length > 0 ? s.points[s.points.length - 1].x : s.points[0]?.x ?? 0; // Find max x for this series
                 const firstPointX = s.points[0]?.x ?? 0;
                 markLineData.push(
                     // Mean line (Note: ECharts markLine is somewhat limited for scatter plots)
                     // We draw a simple horizontal line using yAxis value.
                     // For a line spanning just the series points, more complex logic or a different
                     // approach (like adding a separate 'line' series) might be needed.
                     {
                         name: `${s.name} Mean`,
                         yAxis: s.mean,
                         // Attempting to constrain line - might not work perfectly in scatter
                         // xAxis: lastPointX, 
                         label: {
                             show: true,
                             formatter: `{c} ${data.unit}`, // Use unit from data
                             position: 'end',
                             // Color will be inherited
                         },
                     },
                     // Add trendline if needed (more complex)
                 );
            }

            return {
                name: s.name,
                type: 'scatter',
                // smooth: true, // Not applicable to scatter
                showSymbol: true, // Show points
                symbolSize: 6, // Adjust point size if needed
                label: { show: false }, // Generally too noisy for scatter
                data: seriesPoints,
                markLine: {
                    silent: true, // Non-interactive
                    symbol: ["none", "none"], // No arrows
                    lineStyle: {
                        // Color is inherited
                        width: 2,
                        type: "dashed"
                    },
                    data: markLineData
                }
            };
        });
        
        // Filter legend data to exclude "Mean" lines if they were separate series
        const legendData = data.series.map(s => s.name);


        const options = {
            backgroundColor: "transparent",
            title: {
                text: data.title.toUpperCase(),
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" }
            },
            tooltip: { 
                trigger: "axis", // Or 'item' for scatter points
                axisPointer: { type: "cross" } 
            },
            // Color palette will be applied by lib.js
            grid: {
                top: "10%", bottom: "10%", left: "8%", right: "9%", containLabel: true
            },
            legend: {
                data: legendData, // Use filtered legend names
                bottom: 5,
                orient: "horizontal",
                type: "scroll"
            },
            xAxis: {
                type: "value",
                name: data.x_axis_label,
                nameLocation: "middle",
                nameGap: 30,
                min: 0,
                max: data.total_x_points + 1, // Use max calculated in Rust
                minInterval: 1,
                boundaryGap: false,
                splitLine: { show: false }
            },
            yAxis: {
                type: "value",
                name: data.y_axis_label,
                nameLocation: "middle",
                nameGap: 50,
                splitLine: { show: true },
                max: yMax, // Use calculated P90-based max
                axisLabel: {
                    formatter: function (value) {
                        // Round to 2 decimal places for cleaner display
                        if (typeof value === 'number') {
                            return value.toFixed(2) + ' ' + data.unit;
                        }
                        return value + ' ' + data.unit; // Fallback for non-numeric values
                    }
                }
            },
            series: echartsSeries,
            toolbox: {
                feature: { restore: {}, saveAsImage: {} },
                right: "20px"
            },
             // Base responsive design (can be customized further in templates)
            media: [
                {
                    query: { maxWidth: 768 },
                    option: {
                        legend: {
                            top: "auto",
                            bottom: 5,
                            orient: "horizontal"
                        },
                        grid: {
                            top: "15%",
                            bottom: "18%",
                            left: "10%", 
                            right: "8%"
                        },
                        xAxis: {
                             nameGap: 20,
                             axisLabel: { fontSize: 10 },
                             nameTextStyle: { fontSize: 11 }
                        },
                         yAxis: {
                             nameGap: 35,
                             axisLabel: { fontSize: 10 },
                             nameTextStyle: { fontSize: 11 }
                        }
                    }
                }
            ]
        };

        return options;
    }
};

// =============================
// Summary Chart Generator Module
// =============================

/**
 * Module for generating summary page with multiple bar charts
 * Used for overview of key metrics across all functions
 */
const SummaryCharts = {
    charts: [], // Track all charts for cleanup and resize

    /**
     * Initializes summary page with multiple bar charts
     * @param {Object} summaryData - The summary data from the server
     * @param {string} theme - The theme to use ('light' or 'dark')
     */
    initialize: function(summaryData, theme) {
        // Dispose existing charts
        this.cleanup();

        // Replace the chart containers with summary layout
        this.createSummaryLayout();

        // Create a bar chart for each metric
        summaryData.metrics.forEach((metric, index) => {
            this.createMetricChart(metric, index, theme);
        });
    },

    /**
     * Creates the HTML layout for summary charts
     */
    createSummaryLayout: function() {
        const barChartDom = document.getElementById('chart_bar');
        const lineChartDom = document.getElementById('chart_time');
        
        if (!barChartDom) return;

        // Transform the existing bar chart container to summary layout
        barChartDom.className = 'summary-charts-grid';
        barChartDom.id = 'summary-charts-grid';
        barChartDom.innerHTML = `
        `;

        // Hide the line chart container
        if (lineChartDom) {
            lineChartDom.style.display = 'none';
        }
    },

    /**
     * Creates an individual metric chart
     * @param {Object} metric - Metric data
     * @param {number} index - Chart index for unique IDs
     * @param {string} theme - Theme to use
     */
    createMetricChart: function(metric, index, theme) {
        const grid = document.getElementById('summary-charts-grid');
        if (!grid) return;

        // Create chart container
        const chartContainer = document.createElement('div');
        chartContainer.className = 'summary-chart-item';
        chartContainer.innerHTML = `
            <div class="summary-chart" id="summary-chart-${index}"></div>
            <div class="summary-chart-footer">
                <a href="${metric.link}index.html" class="summary-chart-link">View Details &gt;</a>
            </div>
        `;
        grid.appendChild(chartContainer);

        // Initialize chart
        const chartDom = document.getElementById(`summary-chart-${index}`);
        const chart = echarts.init(chartDom, theme);

        // Create individual series for each function to enable proper coloring and legend
        const series = metric.data.map((dataPoint, index) => ({
            name: dataPoint.name,
            type: 'bar',
            data: [dataPoint.value],
            itemStyle: {
                color: window.DEFAULT_COLOR_PALETTE && index < window.DEFAULT_COLOR_PALETTE.length 
                    ? window.DEFAULT_COLOR_PALETTE[index] 
                    : '#3fb1e3'
            },
            label: {
                show: true,
                position: 'right',
                formatter: `{c} ${metric.unit}`
            }
        }));

        // Create ECharts options directly instead of using BarCharts module
        const options = {
            backgroundColor: "transparent",
            title: {
                text: metric.title.toUpperCase(),
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" }
            },
            tooltip: { 
                trigger: "axis", 
                axisPointer: { type: "shadow" } 
            },
            legend: {
                orient: "horizontal",
                bottom: 5,
                type: "scroll",
                show: true
            },
            grid: {
                left: "5%", 
                top: "10%", 
                right: "5%", 
                bottom: "20%",
                containLabel: true
            },
            xAxis: {
                type: "value",
                name: `${metric.unit === "MB" ? "Memory" : "Duration"} (${metric.unit})`,
                nameLocation: "middle",
                nameGap: 30,
                axisLabel: { formatter: `{value} ${metric.unit}` },
                minInterval: 1
            },
            yAxis: {
                type: "category",
                data: ['Average'],  // Single category since we're showing averages
                inverse: false
            },
            toolbox: {
                feature: { restore: {}, saveAsImage: {} },
                right: "20px"
            },
            series: series
        };

        // Apply color palette
        if (window.DEFAULT_COLOR_PALETTE) {
            options.color = window.DEFAULT_COLOR_PALETTE;
        }

        chart.setOption(options);
        this.charts.push(chart);
    },

    /**
     * Cleans up existing charts
     */
    cleanup: function() {
        this.charts.forEach(chart => {
            if (chart && !chart.isDisposed()) {
                chart.dispose();
            }
        });
        this.charts = [];
    },

    /**
     * Resizes all summary charts
     */
    resize: function() {
        this.charts.forEach(chart => {
            if (chart && !chart.isDisposed()) {
                chart.resize();
            }
        });
    }
};

// ======================
// Initialization on Load
// ======================

// DOMContentLoaded handler
window.addEventListener('DOMContentLoaded', () => {
    // Initialize theme from localStorage or system preference
    const savedTheme = localStorage.getItem('theme');
    const prefersDark = window.matchMedia('(prefers-color-scheme: dark)').matches;
    const initialTheme = savedTheme || (prefersDark ? 'dark' : 'light');
    setTheme(initialTheme); // Don't save on initial load

    // Add listener for OS theme changes
    window.matchMedia('(prefers-color-scheme: dark)').addEventListener('change', (e) => {
        // Only update theme if user hasn't set a manual preference
        if (!localStorage.getItem('theme')) {
            setTheme(e.matches ? 'dark' : 'light');
        }
    });

    // Theme toggle handler
    const themeToggle = document.querySelector('.theme-toggle');
    if (themeToggle) {
        themeToggle.addEventListener('click', () => {
            const currentTheme = root.getAttribute('data-theme');
            setTheme(currentTheme === 'dark' ? 'light' : 'dark', true); // Save preference when toggled
        });
    }

    // Sidebar toggle
    const sidebar = document.getElementById('sidebar');
    const toggleButton = document.getElementById('sidebar-toggle');
    if (toggleButton && sidebar) {
        toggleButton.addEventListener('click', () => {
            sidebar.classList.toggle('sidebar-open');
        });
    }

    // Window resize handler
    window.addEventListener('resize', function() {
        if (barChart) {
            barChart.resize();
        }
        if (lineChart) {
            lineChart.resize();
        }
        if (SummaryCharts && SummaryCharts.charts.length > 0) {
            SummaryCharts.resize();
        }
    });

    // Navigation handler (if needed)
    window.navigateToChartType = function(event) {
        event.preventDefault();
        const linkElement = event.currentTarget;
        const targetGroup = linkElement.dataset.group;
        const targetSubgroup = linkElement.dataset.subgroup;
        // Get the current chart type or default to summary
        const currentChartType = window.currentChartType || 'summary';
        // Use basePath if available, otherwise fallback to root
        const basePath = window.basePath || '/';
        // Get link_suffix from window global (set in template)
        const linkSuffix = window.linkSuffix || '';
        // Construct URL with proper base path, trailing slash, and optional link suffix
        const newUrl = basePath + targetGroup + '/' + targetSubgroup + '/' + currentChartType + '/' + linkSuffix;
        window.location.href = newUrl;
    };
});

// Expose functions globally
window.setTheme = setTheme;
window.prepareScreenshot = prepareScreenshot; 