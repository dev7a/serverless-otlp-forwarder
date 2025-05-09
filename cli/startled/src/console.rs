use crate::stats::calculate_stats;
use colored::*;
use comfy_table::{
    presets::*, Attribute, Cell, CellAlignment, ColumnConstraint, ContentArrangement, Table,
    TableComponent, Width,
};

pub fn print_benchmark_results(function_name: &str, results: &crate::benchmark::BenchmarkResults) {
    const TABLE_WIDTH: u16 = 100;
    const DESCRIPTION_WIDTH: u16 = 22;
    let column_constraints = vec![ColumnConstraint::LowerBoundary(Width::Fixed(
        DESCRIPTION_WIDTH,
    ))];

    let format_value_or_na = |value: f64, unit: &str| -> String {
        if value.is_nan() {
            format!("N/A {}", unit)
        } else {
            format!("{:.2} {}", value, unit)
        }
    };

    if !results.cold_starts.is_empty()
        && results
            .cold_starts
            .iter()
            .any(|m| m.init_duration.is_some())
    {
        println!(
            "{}",
            format!(
                "Function: {} | Cold Start Metrics ({} invocations) | Memory Size: {} MB",
                function_name,
                results.cold_starts.len(),
                results.cold_starts[0].memory_size
            )
            .bright_blue()
            .bold()
        );
        println!("{}", "─".repeat(TABLE_WIDTH as usize).bright_black());
        let mut table = Table::new();
        table
            .load_preset(NOTHING)
            .set_style(TableComponent::MiddleHeaderIntersections, '┴')
            .set_style(TableComponent::BottomBorder, '─')
            .set_style(TableComponent::BottomBorderIntersections, '─')
            .set_style(TableComponent::HeaderLines, '─')
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_width(TABLE_WIDTH)
            .set_header(vec![
                Cell::new("Metric").add_attribute(Attribute::Bold),
                Cell::new("Mean").add_attribute(Attribute::Bold),
                Cell::new("P50").add_attribute(Attribute::Bold),
                Cell::new("P95").add_attribute(Attribute::Bold),
                Cell::new("P99").add_attribute(Attribute::Bold),
                Cell::new("Std Dev").add_attribute(Attribute::Bold),
            ])
            .set_constraints(column_constraints.clone());
        let init_durations: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.init_duration)
            .collect();
        if !init_durations.is_empty() {
            let stats = calculate_stats(&init_durations);
            table.add_row(vec![
                Cell::new("Init Duration"),
                Cell::new(format_value_or_na(stats.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ]);
        }
        let durations: Vec<f64> = results.cold_starts.iter().map(|m| m.duration).collect();
        if !durations.is_empty() {
            let stats = calculate_stats(&durations);
            table.add_row(vec![
                Cell::new("Server Duration"),
                Cell::new(format_value_or_na(stats.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ]);
        }
        let extension_overheads: Vec<f64> = results
            .cold_starts
            .iter()
            .map(|m| m.extension_overhead)
            .collect();
        if !extension_overheads.is_empty() {
            let stats = calculate_stats(&extension_overheads);
            table.add_row(vec![
                Cell::new("Extension Overhead"),
                Cell::new(format_value_or_na(stats.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ]);
        }
        let total_cold_start_durations: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.total_cold_start_duration)
            .collect();
        if !total_cold_start_durations.is_empty() {
            let stats = calculate_stats(&total_cold_start_durations);
            table.add_row(vec![
                Cell::new("Cold Start Duration"),
                Cell::new(format_value_or_na(stats.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ]);
        }
        let billed_durations: Vec<f64> = results
            .cold_starts
            .iter()
            .map(|m| m.billed_duration as f64)
            .collect();
        if !billed_durations.is_empty() {
            let stats = calculate_stats(&billed_durations);
            table.add_row(vec![
                Cell::new("Billed Duration"),
                Cell::new(format_value_or_na(stats.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ]);
            table.add_row(vec![Cell::new("┄".repeat(DESCRIPTION_WIDTH as usize).bright_black())]);
        }

        let response_latencies_cold: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.response_latency_ms)
            .collect();
        if !response_latencies_cold.is_empty() {
            let stats = calculate_stats(&response_latencies_cold);
            table.add_row(vec![
                Cell::new("Response Latency".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let response_durations_cold: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.response_duration_ms)
            .collect();
        if !response_durations_cold.is_empty() {
            let stats = calculate_stats(&response_durations_cold);
            table.add_row(vec![
                Cell::new("Response Duration".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let runtime_overheads_cold: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.runtime_overhead_ms)
            .collect();
        if !runtime_overheads_cold.is_empty() {
            let stats = calculate_stats(&runtime_overheads_cold);
            table.add_row(vec![
                Cell::new("Runtime Overhead".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let rt_done_durations_cold: Vec<f64> = results
            .cold_starts
            .iter()
            .filter_map(|m| m.runtime_done_metrics_duration_ms)
            .collect();
        if !rt_done_durations_cold.is_empty() {
            let stats = calculate_stats(&rt_done_durations_cold);
            table.add_row(vec![
                Cell::new("Runtime Done".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let memory_used_cold: Vec<f64> = results
            .cold_starts
            .iter()
            .map(|m| m.max_memory_used as f64)
            .collect();
        if !memory_used_cold.is_empty() {
            let stats = calculate_stats(&memory_used_cold);
            table.add_row(vec![Cell::new("┄".repeat(DESCRIPTION_WIDTH as usize).bright_black())]);
            table.add_row(vec![
                Cell::new("Memory Used"),
                Cell::new(format_value_or_na(stats.mean, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "MB")).set_alignment(CellAlignment::Right),
            ]);
        }

        println!("{}\n", table);
    }

    if !results.warm_starts.is_empty() {
        println!(
            "{}",
            format!(
                "Function: {} | Warm Start Metrics ({} invocations) | Memory Size: {} MB",
                function_name,
                results.warm_starts.len(),
                results.warm_starts[0].memory_size
            )
            .bright_yellow()
            .bold()
        );
        println!("{}", "─".repeat(TABLE_WIDTH as usize).bright_black());
        let mut table = Table::new();
        table
            .load_preset(NOTHING)
            .set_style(TableComponent::MiddleHeaderIntersections, '┴')
            .set_style(TableComponent::BottomBorder, '─')
            .set_style(TableComponent::BottomBorderIntersections, '─')
            .set_style(TableComponent::HeaderLines, '─')
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_width(TABLE_WIDTH)
            .set_header(vec![
                Cell::new("Metric").add_attribute(Attribute::Bold),
                Cell::new("Mean").add_attribute(Attribute::Bold),
                Cell::new("P50").add_attribute(Attribute::Bold),
                Cell::new("P95").add_attribute(Attribute::Bold),
                Cell::new("P99").add_attribute(Attribute::Bold),
                Cell::new("Std Dev").add_attribute(Attribute::Bold),
            ])
            .set_constraints(column_constraints.clone());
        let durations_warm: Vec<f64> = results.warm_starts.iter().map(|m| m.duration).collect();
        let stats_warm_duration = calculate_stats(&durations_warm);
        table.add_row(vec![
            Cell::new("Server Duration"),
            Cell::new(format_value_or_na(stats_warm_duration.mean, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_duration.p50, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_duration.p95, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_duration.p99, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_duration.std_dev, "ms")).set_alignment(CellAlignment::Right),
        ]);
        let extension_overheads_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .map(|m| m.extension_overhead)
            .collect();
        let stats_warm_ext_overhead = calculate_stats(&extension_overheads_warm);
        table.add_row(vec![
            Cell::new("Extension Overhead"),
            Cell::new(format_value_or_na(stats_warm_ext_overhead.mean, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_ext_overhead.p50, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_ext_overhead.p95, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_ext_overhead.p99, "ms")).set_alignment(CellAlignment::Right),
            Cell::new(format_value_or_na(stats_warm_ext_overhead.std_dev, "ms")).set_alignment(CellAlignment::Right),
        ]);
        let billed_durations_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .map(|m| m.billed_duration as f64)
            .collect();
        let stats_warm_billed = calculate_stats(&billed_durations_warm);
        table.add_rows(vec![
            vec![
                Cell::new("Billed Duration"),
                Cell::new(format_value_or_na(stats_warm_billed.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats_warm_billed.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats_warm_billed.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats_warm_billed.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats_warm_billed.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ],
            vec![Cell::new("┄".repeat(DESCRIPTION_WIDTH as usize).bright_black())],
        ]);

        let response_latencies_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .filter_map(|m| m.response_latency_ms)
            .collect();
        if !response_latencies_warm.is_empty() {
            let stats = calculate_stats(&response_latencies_warm);
            table.add_row(vec![
                Cell::new("Response Latency".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let response_durations_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .filter_map(|m| m.response_duration_ms)
            .collect();
        if !response_durations_warm.is_empty() {
            let stats = calculate_stats(&response_durations_warm);
            table.add_row(vec![
                Cell::new("Response Duration".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let runtime_overheads_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .filter_map(|m| m.runtime_overhead_ms)
            .collect();
        if !runtime_overheads_warm.is_empty() {
            let stats = calculate_stats(&runtime_overheads_warm);
            table.add_row(vec![
                Cell::new("Runtime Overhead".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let rt_done_durations_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .filter_map(|m| m.runtime_done_metrics_duration_ms)
            .collect();
        if !rt_done_durations_warm.is_empty() {
            let stats = calculate_stats(&rt_done_durations_warm);
            table.add_row(vec![
                Cell::new("Runtime Done".bright_black()),
                Cell::new(format_value_or_na(stats.mean, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms").bright_black()).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms").bright_black()).set_alignment(CellAlignment::Right),
            ]);
        }

        let memory_used_warm: Vec<f64> = results
            .warm_starts
            .iter()
            .map(|m| m.max_memory_used as f64)
            .collect();
        if !memory_used_warm.is_empty() {
            let stats = calculate_stats(&memory_used_warm);
            table.add_row(vec![Cell::new("┄".repeat(DESCRIPTION_WIDTH as usize).bright_black())]);
            table.add_row(vec![
                Cell::new("Memory Used"),
                Cell::new(format_value_or_na(stats.mean, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "MB")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "MB")).set_alignment(CellAlignment::Right),
            ]);
        }

        println!("{}\n", table);
    }

    if !results.client_measurements.is_empty() {
        println!(
            "{}",
            format!(
                "Function: {} | Client Metrics ({} invocations) | Memory Size: {} MB",
                function_name,
                results.client_measurements.len(),
                results.client_measurements[0].memory_size
            )
            .bright_cyan()
            .bold()
        );
        println!("{}", "─".repeat(TABLE_WIDTH as usize).bright_black());
        let mut table = Table::new();
        table
            .load_preset(NOTHING)
            .set_style(TableComponent::MiddleHeaderIntersections, '┴')
            .set_style(TableComponent::BottomBorder, '─')
            .set_style(TableComponent::BottomBorderIntersections, '─')
            .set_style(TableComponent::HeaderLines, '─')
            .set_content_arrangement(ContentArrangement::DynamicFullWidth)
            .set_width(TABLE_WIDTH)
            .set_header(vec![
                Cell::new("Metric").add_attribute(Attribute::Bold),
                Cell::new("Mean").add_attribute(Attribute::Bold),
                Cell::new("P50").add_attribute(Attribute::Bold),
                Cell::new("P95").add_attribute(Attribute::Bold),
                Cell::new("P99").add_attribute(Attribute::Bold),
                Cell::new("Std Dev").add_attribute(Attribute::Bold),
            ])
            .set_constraints(column_constraints.clone());
        let client_durations: Vec<f64> = results
            .client_measurements
            .iter()
            .map(|m| m.client_duration)
            .collect();
        if !client_durations.is_empty() {
            let stats = calculate_stats(&client_durations);
            table.add_row(vec![
                Cell::new("Client Duration"),
                Cell::new(format_value_or_na(stats.mean, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p50, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p95, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.p99, "ms")).set_alignment(CellAlignment::Right),
                Cell::new(format_value_or_na(stats.std_dev, "ms")).set_alignment(CellAlignment::Right),
            ]);
        }
        println!("{}\n", table);
    }
}
