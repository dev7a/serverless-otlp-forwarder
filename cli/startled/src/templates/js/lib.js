/**
 * Consolidated JavaScript library for the startled CLI report generator
 * 
 * This file contains all the JavaScript functionality needed for the benchmark reports:
 * 1. Base functionality for theming, UI behavior, and chart initialization
 * 2. Bar chart generation for metrics like cold start, warm start, and memory usage
 * 3. Scatter chart generation for time-series data like client duration over time
 * 4. Memory scaling charts for performance across different memory configurations
 * 5. Summary charts for overview pages with multiple metrics
 * 6. Series highlighting and interactive tooltip functionality
 * 
 * Users can customize this file to change the appearance and behavior of the reports.
 * When providing a custom template directory with --template-dir, place a modified
 * version of this file at: <your-template-dir>/js/lib.js
 */

/**
 * Default color palette for charts (can be customized)
 * @type {string[]}
 */
window.DEFAULT_COLOR_PALETTE = [
    "#3fb1e3",
    "#6be6c1",
    "#a0a7e6",
    "#7b936c",
    "#96dee8",
    "#e396d0",
    "#e3b396",
    "#966be3",
    "#626c91",
    "#96e3a7"
];

/**
 * Chart styling constants
 * @type {Object}
 */
const CHART_CONSTANTS = {
    // Highlighting and emphasis
    BLUR_OPACITY: 0.1,
    EMPHASIS_LINE_WIDTH: 4,
    NORMAL_LINE_WIDTH: 2,
    EMPHASIS_SYMBOL_SIZE: 10,
    NORMAL_SYMBOL_SIZE: 8,
    SCATTER_SYMBOL_SIZE: 6,
    
    // Colors and styling
    HIGHLIGHT_COLOR: '#3fb1e3',
    HIGHLIGHT_BACKGROUND: 'rgba(63, 177, 227, 0.2)',
    HIGHLIGHT_BORDER: '3px solid #3fb1e3',
    MUTED_OPACITY: 0.8,
    
    // Theme-based background colors
    DARK_BACKGROUND: '#000000',   // Black background for dark theme
    LIGHT_BACKGROUND: '#ffffff',  // White background for light theme
    
    // Animation
    ANIMATION_DURATION: 1000,
    ANIMATION_EASING: 'cubicOut',
    
    // Layout
    CHART_RESIZE_DELAY: 200
};

// ===============================
// Core UI and Setup Functionality
// ===============================

/**
 * Centralized chart management system
 * @type {Object}
 */
const ChartManager = {
    instances: new Map(),
    
    /**
     * Registers a chart instance
     * @param {string} id - Unique identifier for the chart
     * @param {Object} chart - ECharts instance
     */
    register: function(id, chart) {
        this.instances.set(id, chart);
    },
    
    /**
     * Disposes a specific chart
     * @param {string} id - Chart identifier
     */
    dispose: function(id) {
        const chart = this.instances.get(id);
        if (chart && !chart.isDisposed()) {
            chart.dispose();
        }
        this.instances.delete(id);
    },
    
    /**
     * Disposes all charts
     */
    disposeAll: function() {
        try {
            this.instances.forEach((chart, id) => {
                try {
                                if (chart && !chart.isDisposed()) {
                chart.dispose();
            }
        } catch (error) {
            // Silently handle disposal errors
        }
            });
            this.instances.clear();
        } catch (error) {
            // Silently handle disposal errors
        }
    },
    
    /**
     * Gets a chart by ID
     * @param {string} id - Chart identifier
     * @returns {Object|null} ECharts instance or null
     */
    get: function(id) {
        return this.instances.get(id) || null;
    },
    
    /**
     * Resizes all charts
     */
    resizeAll: function() {
        this.instances.forEach((chart, id) => {
            if (chart && !chart.isDisposed()) {
                chart.resize();
            }
        });
    }
};

/**
 * Legacy global chart instances (for backward compatibility)
 * @type {echarts.ECharts|null}
 */
let barChart;
let lineChart;

/**
 * Document root element reference
 * @type {HTMLElement}
 */
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
    
    // Dispose all existing charts using centralized manager
    ChartManager.disposeAll();
    
    // Clear legacy references
    barChart = null;
    lineChart = null;
    
    // Clear legacy arrays
    if (window.memoryScalingCharts) {
        window.memoryScalingCharts = [];
    }
    
    // Restore original DOM structure before recreating charts
    restoreOriginalChartDOM();
    
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

    const domElements = getChartDOMElements();
    
    // Handle special case for summary page with metrics data format
    if (window.chartType === "summary" && window.currentChartSpecificData.metrics) {
        SummaryCharts.createSummaryLayout();
        
        window.currentChartSpecificData.metrics.forEach((metric, index) => {
            SummaryCharts.createMetricChart(metric, index, theme);
        });
        return;
    }

    // Use registry pattern for chart type handling
    const chartType = Object.keys(window.currentChartSpecificData)[0];
    const chartData = window.currentChartSpecificData[chartType];
    const handler = ChartHandlers[chartType];
    
    if (handler) {
        handler(theme, domElements, chartData);
    } else {
        console.error('Unknown chart data format:', chartType);
    }
}

/**
 * Prepares the page for taking screenshots
 * @param {string} theme - The theme to use for the screenshot
 */
function prepareScreenshot(theme) {
    try {
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
            try {
                ChartManager.resizeAll();
            } catch (error) {
                // Silently handle chart resize errors
            }
        }, CHART_CONSTANTS.CHART_RESIZE_DELAY);
        
    } catch (error) {
        // Fallback: just set the theme without chart operations
        root.setAttribute('data-theme', theme);
    }
}

// ============================
// Bar Chart Generator Module
// using Apache echarts.js (https://echarts.apache.org/en/index.html)
// ============================

/**
 * Module for generating bar chart options
 * Used for visualizing metrics like cold start times, memory usage, etc.
 * @namespace BarCharts
 */
const BarCharts = {
    /**
     * Generates ECharts options for bar charts
     * @param {Object} chartSpecificData - The chart data from the server
     * @param {string} theme - The current theme ('light' or 'dark')
     * @returns {Object} ECharts options object
     */
    generateOptions: function(chartSpecificData, theme = 'light') {
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
            emphasis: {
                focus: 'series',
                blurScope: 'coordinateSystem'
            },
            blur: {
                itemStyle: {
                    opacity: CHART_CONSTANTS.BLUR_OPACITY
                }
            },
            data: s.values.map((value, index) => ({
                value: value,
                name: data.y_axis_categories[index] // Assumes values align with categories
            }))
        }));

        const options = {
            backgroundColor: getThemeBackgroundColor(theme),
            title: {
                text: data.title.toUpperCase(),
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" }
            },
            tooltip: { 
                order: 'valueDesc',
                trigger: "axis", 
                axisPointer: { type: "shadow" },
                formatter: function(params) {
                    if (!params || params.length === 0) return '';
                    
                    let tooltip = `<strong>${params[0].name}</strong><br/>`;
                    
                    // Sort by value (descending) to match order: 'valueDesc'
                    const sortedParams = [...params].sort((a, b) => b.value - a.value);
                    
                    // Get the hovered series name from global state
                    const hoveredSeriesName = window._currentHoveredSeries;
                    
                    sortedParams.forEach((param, index) => {
                        // Highlight the currently hovered series
                        const isHovered = hoveredSeriesName && param.seriesName === hoveredSeriesName;
                        const style = isHovered ? 
                            `font-weight: bold; background-color: ${CHART_CONSTANTS.HIGHLIGHT_BACKGROUND}; padding: 2px 4px; border-radius: 3px; border-left: ${CHART_CONSTANTS.HIGHLIGHT_BORDER}; margin: 1px 0;` : 
                            `opacity: ${CHART_CONSTANTS.MUTED_OPACITY}; margin: 1px 0;`;
                        
                        tooltip += `<div style="${style}">`;
                        tooltip += `${param.marker} ${param.seriesName}: `;
                        tooltip += `<strong>${param.value} ${data.unit}</strong>`;
                        tooltip += `</div>`;
                    });
                    
                    return tooltip;
                }
            },
            // Color palette will be applied by setupChart function
            legend: {
                orient: "horizontal",
                bottom: 5
            },
            grid: [{
                left: "30", top: "50", right: "50", bottom: "85",
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
                feature: { saveAsImage: {} },
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
// Scatter Chart Generator Module
// ============================

/**
 * Module for generating scatter chart options
 * Used for time-series data like client duration over time
 * @namespace ScatterCharts
 */
const ScatterCharts = {
    /**
     * Generates ECharts options for scatter charts
     * @param {Object} chartSpecificData - The chart data from the server
     * @param {string} theme - The current theme ('light' or 'dark')
     * @returns {Object} ECharts options object
     */
    generateOptions: function(chartSpecificData, theme = 'light') {
        if (!chartSpecificData || !chartSpecificData.Line) {
            console.error("Invalid data format for scatter chart generator:", chartSpecificData);
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
                symbolSize: CHART_CONSTANTS.SCATTER_SYMBOL_SIZE, // Adjust point size if needed
                label: { show: false }, // Generally too noisy for scatter
                data: seriesPoints,
                markLine: {
                    silent: true, // Non-interactive
                    symbol: ["none", "none"], // No arrows
                    lineStyle: {
                        // Color is inherited
                        width: CHART_CONSTANTS.NORMAL_LINE_WIDTH,
                        type: "dashed"
                    },
                    data: markLineData
                }
            };
        });
        
        // Filter legend data to exclude "Mean" lines if they were separate series
        const legendData = data.series.map(s => s.name);

        const options = {
            backgroundColor: getThemeBackgroundColor(theme),
            title: {
                text: data.title.toUpperCase(),
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" }
            },
            tooltip: { 
                order: 'valueDesc',
                trigger: "axis", // Or 'item' for scatter points
                axisPointer: { type: "cross" } 
            },
            // Color palette will be applied by setupChart function
            grid: {
                top: "30", bottom: "85", left: "50", right: "70", containLabel: true
            },
            legend: {
                data: legendData, // Use filtered legend names
                bottom: 5,
                orient: "horizontal"
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
                feature: { dataZoom: { yAxisIndex: 'none' }, saveAsImage: {} },
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

// ============================
// Memory Scaling Chart Generator Module
// ============================

/**
 * Module for generating memory scaling charts
 * Used for visualizing performance across different memory configurations
 * @namespace MemoryScalingCharts
 */
const MemoryScalingCharts = {
    /**
     * Generates ECharts options for memory scaling line charts
     * @param {Object} chartSpecificData - The chart data from the server
     * @param {string} theme - The current theme ('light' or 'dark')
     * @returns {Object} ECharts options object
     */
    generateOptions: function(chartSpecificData, theme = 'light') {
        if (!chartSpecificData || !chartSpecificData.MemoryScaling) {
            console.error("Invalid data format for memory scaling chart generator:", chartSpecificData);
            return {}; // Return empty options on error
        }
        const data = chartSpecificData.MemoryScaling;

        // Transform series data for ECharts
        const echartsSeries = data.series.map(s => {
            // Extract memory sizes and values
            const memoryLabels = s.points.map(p => `${p.memory_mb} MB`);
            const values = s.points.map(p => p.value);
            
            return {
                name: s.name,
                type: 'line',
                data: values,
                smooth: true,
                symbol: 'circle',
                symbolSize: CHART_CONSTANTS.NORMAL_SYMBOL_SIZE,
                emphasis: {
                    focus: 'series',
                    blurScope: 'coordinateSystem',
                    lineStyle: {
                        width: CHART_CONSTANTS.EMPHASIS_LINE_WIDTH
                    }
                },
                blur: {
                    lineStyle: {
                        opacity: CHART_CONSTANTS.BLUR_OPACITY
                    }
                },
                lineStyle: {
                    width: CHART_CONSTANTS.NORMAL_LINE_WIDTH
                }
            };
        });

        // Get unique memory sizes for x-axis
        const memoryConfigs = [...new Set(data.series.flatMap(s => s.points.map(p => p.memory_mb)))]
            .sort((a, b) => a - b)
            .map(mb => `${mb} MB`);

        const options = {
            backgroundColor: getThemeBackgroundColor(theme),
            title: {
                text: data.title.toUpperCase(),
                subtext: data.subtitle,
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" },
                subtextStyle: { fontSize: 12, color: "#999" }
            },
            tooltip: { 
                order: 'valueDesc',
                trigger: "axis",
                axisPointer: { 
                    type: "cross",
                },
                formatter: function(params) {
                    if (!params || params.length === 0) return '';
                    
                    let tooltip = `<strong>${params[0].axisValue}</strong><br/>`;
                    
                    // Sort by value (descending) to match order: 'valueDesc'
                    const sortedParams = [...params].sort((a, b) => b.value - a.value);
                    
                    // Get the hovered series name from global state
                    const hoveredSeriesName = window._currentHoveredSeries;
                    
                    sortedParams.forEach((param, index) => {
                        // Highlight the currently hovered series
                        const isHovered = hoveredSeriesName && param.seriesName === hoveredSeriesName;
                        const style = isHovered ? 
                            `font-weight: bold; background-color: ${CHART_CONSTANTS.HIGHLIGHT_BACKGROUND}; padding: 2px 4px; border-radius: 3px; border-left: ${CHART_CONSTANTS.HIGHLIGHT_BORDER}; margin: 1px 0;` : 
                            `opacity: ${CHART_CONSTANTS.MUTED_OPACITY}; margin: 1px 0;`;
                        
                        tooltip += `<div style="${style}">`;
                        tooltip += `${param.marker} ${param.seriesName}: `;
                        tooltip += `<strong>${param.value.toFixed(2)} ${data.unit}</strong>`;
                        tooltip += `</div>`;
                    });
                    
                    return tooltip;
                }
            },
            legend: {
                bottom: 5,
                orient: "horizontal",
                selectedMode: 'multiple',
            },
            grid: {
                left: "50", 
                top: "50", 
                right: "30", 
                bottom: "70",
                containLabel: true
            },
            xAxis: {
                type: "category",
                data: memoryConfigs,
                name: data.x_axis_label,
                nameLocation: "middle",
                nameGap: 30,
                boundaryGap: true,
                axisLabel: {
                    rotate: 0
                }
            },
            yAxis: {
                type: "value",
                name: data.y_axis_label,
                nameLocation: "middle",
                nameGap: 60,
                splitLine: { show: true },
                axisLabel: {
                    formatter: function(value) {
                        if (data.unit === "GB-seconds per Million") {
                            return value.toLocaleString();
                        } else if (data.unit === "%") {
                            return value.toFixed(1) + '%';
                        } else {
                            return value.toFixed(0);
                        }
                    }
                }
            },
            series: echartsSeries,
            toolbox: {
                feature: { dataZoom: { xAxisIndex: 'none' }, saveAsImage: {} },
            },
            // Add visual effects
            animationDuration: CHART_CONSTANTS.ANIMATION_DURATION,
            animationEasing: CHART_CONSTANTS.ANIMATION_EASING,
            
            // Responsive design
            media: [
                {
                    query: { maxWidth: 768 },
                    option: {
                        legend: {
                            orient: "vertical",
                            right: 0,
                            top: "center"
                        },
                        grid: {
                            left: "15%",
                            right: "25%",
                            top: "20%",
                            bottom: "15%"
                        },
                        xAxis: {
                            nameGap: 20,
                            axisLabel: { 
                                fontSize: 10,
                                rotate: 45
                            },
                            nameTextStyle: { fontSize: 11 }
                        },
                        yAxis: {
                            nameGap: 40,
                            axisLabel: { fontSize: 10 },
                            nameTextStyle: { fontSize: 11 }
                        },
                        series: echartsSeries.map(s => ({
                            ...s,
                            symbolSize: CHART_CONSTANTS.SCATTER_SYMBOL_SIZE,
                            lineStyle: { width: 1 }
                        }))
                    }
                }
            ]
        };

        return options;
    },
    
    /**
     * Initializes multiple memory scaling charts on a single page
     * @param {Object} summaryData - The memory scaling summary data
     * @param {string} theme - The theme to use ('light' or 'dark')
     */
    initializeMultiple: function(summaryData, theme) {
        // Look for existing container or create from original
        let barChartDom = document.getElementById('memory-scaling-charts-grid') || document.getElementById('chart_bar');
        const lineChartDom = document.getElementById('chart_time');
        
        if (!barChartDom) {
            console.error('No suitable container found for memory scaling charts');
            return;
        }
        
        // Transform to grid layout for multiple charts
        barChartDom.className = 'summary-charts-grid';
        barChartDom.id = 'memory-scaling-charts-grid';
        barChartDom.innerHTML = "";
        
        // Hide line chart container
        if (lineChartDom) {
            lineChartDom.style.display = 'none';
        }
        
        // Create a chart for each metric
        summaryData.charts.forEach((chartData, index) => {
            // Create chart container
            const chartContainer = document.createElement('div');
            chartContainer.className = 'summary-chart-item';
            chartContainer.innerHTML = `
                <div class="summary-chart" id="memory-scaling-chart-${index}"></div>
            `;
            barChartDom.appendChild(chartContainer);
            
            // Initialize the chart
            const chartDom = document.getElementById(`memory-scaling-chart-${index}`);
            const chart = echarts.init(chartDom, theme);
            
            // Register with chart manager
            ChartManager.register(`memoryScalingChart-${index}`, chart);
            
            // Generate options for this specific chart
            const singleChartData = {
                MemoryScaling: chartData
            };
            const options = this.generateOptions(singleChartData, theme);
            
            // Apply color palette if not set
            if (options && typeof options.color === 'undefined' && window.DEFAULT_COLOR_PALETTE) {
                options.color = window.DEFAULT_COLOR_PALETTE;
            }
            
            chart.setOption(options);
            
            // Add tooltip tracking for series highlighting
            addChartHighlighting(chart);
            
            // Store chart reference for legacy compatibility
            if (!window.memoryScalingCharts) {
                window.memoryScalingCharts = [];
            }
            window.memoryScalingCharts.push(chart);
        });
        
        // Note: Chart resizing is now handled centrally by ChartManager
    }
};

// =============================
// Summary Chart Generator Module
// =============================

/**
 * Module for generating summary page with multiple bar charts
 * Used for overview of key metrics across all functions
 * @namespace SummaryCharts
 */
const SummaryCharts = {

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
        barChartDom.innerHTML = "";

        // Hide the line chart container
        if (lineChartDom) {
            lineChartDom.style.display = 'none';
        }
    },

    /**
     * Creates an individual metric chart
     * @param {Object} metric - Metric data
     * @param {number} index - Chart index for unique IDs
     * @param {string} theme - Theme to use ('light' or 'dark')
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
        
        // Register with chart manager
        ChartManager.register(`summaryChart-${index}`, chart);

        // Create individual series for each function to enable proper coloring and legend
        const series = metric.data.map((dataPoint, index) => ({
            name: dataPoint.name,
            type: 'bar',
            data: [dataPoint.value],
            emphasis: {
                focus: 'series',
                blurScope: 'coordinateSystem'
            },
            blur: {
                itemStyle: {
                    opacity: CHART_CONSTANTS.BLUR_OPACITY
                }
            },
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

        // Create ECharts options with enhanced tooltip
        const options = {
            backgroundColor: getThemeBackgroundColor(theme),
            title: {
                text: metric.title.toUpperCase(),
                top: "5",
                left: "center",
                textStyle: { fontWeight: "light", color: "#666" }
            },
            tooltip: { 
                order: 'valueDesc',
                trigger: "axis", 
                axisPointer: { type: "shadow" },
                formatter: function(params) {
                    if (!params || params.length === 0) return '';
                    
                    let tooltip = `<strong>${params[0].name}</strong><br/>`;
                    
                    // Sort by value (descending) to match order: 'valueDesc'
                    const sortedParams = [...params].sort((a, b) => b.value - a.value);
                    
                    // Get the hovered series name from global state
                    const hoveredSeriesName = window._currentHoveredSeries;
                    
                    sortedParams.forEach((param, index) => {
                        // Highlight the currently hovered series
                        const isHovered = hoveredSeriesName && param.seriesName === hoveredSeriesName;
                        const style = isHovered ? 
                            `font-weight: bold; background-color: ${CHART_CONSTANTS.HIGHLIGHT_BACKGROUND}; padding: 2px 4px; border-radius: 3px; border-left: ${CHART_CONSTANTS.HIGHLIGHT_BORDER}; margin: 1px 0;` : 
                            `opacity: ${CHART_CONSTANTS.MUTED_OPACITY}; margin: 1px 0;`;
                        
                        tooltip += `<div style="${style}">`;
                        tooltip += `${param.marker} ${param.seriesName}: `;
                        tooltip += `<strong>${param.value} ${metric.unit}</strong>`;
                        tooltip += `</div>`;
                    });
                    
                    return tooltip;
                }
            },
            legend: {
                orient: "horizontal",
                bottom: 5,
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
                feature: { saveAsImage: {} },
                right: "20px"
            },
            series: series
        };

        // Apply color palette
        if (window.DEFAULT_COLOR_PALETTE) {
            options.color = window.DEFAULT_COLOR_PALETTE;
        }

        chart.setOption(options);
        
        // Add tooltip tracking for series highlighting
        addChartHighlighting(chart);
    },

    /**
     * Cleans up existing charts (deprecated - using ChartManager now)
     */
    cleanup: function() {
        // This method is kept for compatibility but ChartManager handles cleanup
    }
};

// ===============================
// Common Chart Utilities
// ===============================

/**
 * Gets the appropriate background color based on theme
 * @param {string} theme - The current theme ('light' or 'dark')
 * @returns {string} The background color for the theme
 */
function getThemeBackgroundColor(theme) {
    return theme === 'dark' ? CHART_CONSTANTS.DARK_BACKGROUND : CHART_CONSTANTS.LIGHT_BACKGROUND;
}

/**
 * Applies common chart setup including color palette and optional highlighting
 * @param {Object} chartInstance - ECharts instance
 * @param {Object} options - Chart options
 * @param {boolean} enableHighlighting - Whether to add series highlighting
 * @returns {Object} The chart instance
 */
function setupChart(chartInstance, options, enableHighlighting = true) {
    // Apply color palette if not already set
    if (options && typeof options.color === 'undefined' && window.DEFAULT_COLOR_PALETTE) {
        options.color = window.DEFAULT_COLOR_PALETTE;
    }
    
    chartInstance.setOption(options);
    
    // Add highlighting if enabled
    if (enableHighlighting) {
        addChartHighlighting(chartInstance);
    }
    
    return chartInstance;
}

/**
 * Adds tooltip tracking and legend highlighting to a chart
 * @param {Object} chart - ECharts instance
 */
function addChartHighlighting(chart) {
    chart.on('mouseover', 'series', function(params) {
        // Store hovered series for tooltip highlighting
        window._currentHoveredSeries = params.seriesName;
        
        // Trigger legend hover effect for the hovered series
        chart.dispatchAction({
            type: 'legendHover',
            name: params.seriesName
        });
    });
    
    chart.on('mouseout', 'series', function(params) {
        // Clear hovered series tracking
        window._currentHoveredSeries = null;
        
        // Clear legend hover effect
        chart.dispatchAction({
            type: 'legendUnHover',
            name: params.seriesName
        });
    });
}

/**
 * Restores the original DOM structure for charts
 */
function restoreOriginalChartDOM() {
    try {
        // Restore memory scaling charts grid back to original bar chart container
        const memoryScalingGrid = document.getElementById('memory-scaling-charts-grid');
        if (memoryScalingGrid) {
            memoryScalingGrid.id = 'chart_bar';
            memoryScalingGrid.className = '';
            memoryScalingGrid.innerHTML = '';
        }
        
        // Restore summary charts grid back to original bar chart container
        const summaryGrid = document.getElementById('summary-charts-grid');
        if (summaryGrid) {
            summaryGrid.id = 'chart_bar';
            summaryGrid.className = '';
            summaryGrid.innerHTML = '';
        }
        
        // Make sure line chart container is visible
        const lineChartDom = document.getElementById('chart_time');
        if (lineChartDom) {
            lineChartDom.style.display = '';
        }
            } catch (error) {
            // Silently handle DOM restoration errors
        }
}

/**
 * Gets DOM elements needed for chart initialization
 * @returns {Object} Object containing chart DOM elements
 */
function getChartDOMElements() {
    return {
        barChart: document.getElementById('chart_bar'),
        lineChart: document.getElementById('chart_time'),
        summaryGrid: document.getElementById('summary-charts-grid')
    };
}

// ===============================
// Chart Type Handlers
// ===============================

/**
 * Registry of chart type handlers for different data formats
 * @namespace ChartHandlers
 */
const ChartHandlers = {
    /**
     * Handles combined charts (bar + line)
     * @param {string} theme - The theme to use ('light' or 'dark')
     * @param {Object} domElements - DOM elements for chart containers
     * @param {Object} data - Combined chart data containing bar and line data
     */
    Combined: function(theme, domElements, data) {
        const { barChart: barChartDom, lineChart: lineChartDom } = domElements;
        
        if (!barChartDom) {
            console.error("Bar chart DOM element not found for combined chart.");
            return;
        }

        // Initialize bar chart
        barChart = echarts.init(barChartDom, theme);
        ChartManager.register('barChart', barChart);
        const barOptions = BarCharts.generateOptions({ Bar: data.bar }, theme);
        setupChart(barChart, barOptions);
        
        // Initialize line chart if DOM element exists
        if (lineChartDom) {
            lineChart = echarts.init(lineChartDom, theme);
            ChartManager.register('lineChart', lineChart);
            const lineOptions = ScatterCharts.generateOptions({ Line: data.line }, theme);
            setupChart(lineChart, lineOptions);
        }
    },

    /**
     * Handles standalone bar charts
     * @param {string} theme - The theme to use ('light' or 'dark')
     * @param {Object} domElements - DOM elements for chart containers
     * @param {Object} data - Bar chart data
     */
    Bar: function(theme, domElements, data) {
        const { barChart: barChartDom } = domElements;
        
        if (!barChartDom) {
            console.error("Bar chart DOM element not found.");
            return;
        }

        barChart = echarts.init(barChartDom, theme);
        ChartManager.register('barChart', barChart);
        const options = BarCharts.generateOptions({ Bar: data }, theme);
        setupChart(barChart, options);
    },

    /**
     * Handles standalone scatter charts
     * @param {string} theme - The theme to use ('light' or 'dark')
     * @param {Object} domElements - DOM elements for chart containers
     * @param {Object} data - Line/scatter chart data
     */
    Line: function(theme, domElements, data) {
        const { lineChart: lineChartDom } = domElements;
        
        if (!lineChartDom) {
            console.error("Line chart DOM element not found.");
            return;
        }

        lineChart = echarts.init(lineChartDom, theme);
        ChartManager.register('lineChart', lineChart);
        const options = ScatterCharts.generateOptions({ Line: data }, theme);
        setupChart(lineChart, options);
    },

    /**
     * Handles memory scaling charts
     * @param {string} theme - The theme to use ('light' or 'dark')
     * @param {Object} domElements - DOM elements for chart containers
     * @param {Object} data - Memory scaling chart data
     */
    MemoryScaling: function(theme, domElements, data) {
        const { lineChart: lineChartDom, barChart: barChartDom } = domElements;
        const chartDom = lineChartDom || barChartDom;
        
        if (!chartDom) {
            console.error("No suitable DOM element found for memory scaling chart.");
            return;
        }

        lineChart = echarts.init(chartDom, theme);
        ChartManager.register('memoryScalingChart', lineChart);
        const options = MemoryScalingCharts.generateOptions({ MemoryScaling: data }, theme);
        setupChart(lineChart, options);
    },

    /**
     * Handles memory scaling summary (multiple charts)
     * @param {string} theme - The theme to use ('light' or 'dark')
     * @param {Object} domElements - DOM elements for chart containers
     * @param {Object} data - Memory scaling summary data
     */
    MemoryScalingSummary: function(theme, domElements, data) {
        MemoryScalingCharts.initializeMultiple(data, theme);
    },

    /**
     * Handles summary charts
     * @param {string} theme - The theme to use ('light' or 'dark')
     * @param {Object} domElements - DOM elements for chart containers
     * @param {Object} data - Summary chart data
     */
    Summary: function(theme, domElements, data) {
        SummaryCharts.initialize(data, theme);
    }
};

// ======================
// Initialization on Load
// ======================

/**
 * DOMContentLoaded event handler
 * Initializes the entire report interface including theme, charts, and event listeners
 */
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
        ChartManager.resizeAll();
    });

    /**
     * Navigation handler for chart type links
     * @param {Event} event - Click event
     */
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

/**
 * Expose functions globally for external use
 */
try {
    window.setTheme = setTheme;
    window.prepareScreenshot = prepareScreenshot;
    
    // Ensure prepareScreenshot is always available, even if there are errors
    if (typeof window.prepareScreenshot !== 'function') {
        window.prepareScreenshot = function(theme) {
            try {
                document.documentElement.setAttribute('data-theme', theme);
            } catch (e) {
                // Silently handle fallback errors
            }
        };
    }
} catch (error) {
    // Emergency fallback
    window.prepareScreenshot = function(theme) {
        try {
            document.documentElement.setAttribute('data-theme', theme);
        } catch (e) {
            // Silently handle fallback errors
        }
    };
} 