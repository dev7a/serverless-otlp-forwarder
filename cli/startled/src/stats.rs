use statrs::statistics::{Data, Distribution, OrderStatistics};

pub struct MetricsStats {
    pub mean: f64,
    pub p50: f64,
    pub p95: f64,
    pub p99: f64,
    pub std_dev: f64,
}

pub fn calculate_stats(values: &[f64]) -> MetricsStats {
    if values.is_empty() {
        return MetricsStats {
            mean: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
            std_dev: 0.0,
        };
    }
    if values.len() < 2 {
        let val = values[0];
        return MetricsStats {
            mean: val,
            p50: val,
            p95: val,
            p99: val,
            std_dev: 0.0,
        };
    }

    let mut data = Data::new(values.to_vec());
    let mean = data.mean().unwrap_or(f64::NAN);
    let p50 = data.percentile(50);
    let p95 = data.percentile(95);
    let p99 = data.percentile(99);
    let std_dev = data.std_dev().unwrap_or(f64::NAN);

    MetricsStats {
        mean,
        p50,
        p95,
        p99,
        std_dev,
    }
}

/// Calculate statistics for cold start init duration
pub fn calculate_cold_start_init_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = cold_starts.iter().map(|m| m.init_duration).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}

/// Calculate statistics for cold start server duration
pub fn calculate_cold_start_server_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = cold_starts.iter().map(|m| m.duration).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}

/// Calculate statistics for warm start metrics
pub fn calculate_warm_start_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
    field: fn(&crate::types::WarmStartMetrics) -> f64,
) -> Option<(f64, f64, f64, f64)> {
    if warm_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = warm_starts.iter().map(field).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}

/// Calculate statistics for client metrics
pub fn calculate_client_stats(
    client_measurements: &[crate::types::ClientMetrics],
) -> Option<(f64, f64, f64, f64)> {
    if client_measurements.is_empty() {
        return None;
    }
    let durations: Vec<f64> = client_measurements
        .iter()
        .map(|m| m.client_duration)
        .collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}

/// Calculate memory usage statistics
pub fn calculate_memory_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64)> {
    if warm_starts.is_empty() {
        return None;
    }
    let memory: Vec<f64> = warm_starts
        .iter()
        .map(|m| m.max_memory_used as f64)
        .collect();
    let stats = calculate_stats(&memory);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}

/// Calculate statistics for cold start extension overhead
pub fn calculate_cold_start_extension_overhead_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let overheads: Vec<f64> = cold_starts.iter().map(|m| m.extension_overhead).collect();
    let stats = calculate_stats(&overheads);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}

/// Calculate statistics for cold start total cold start duration
pub fn calculate_cold_start_total_duration_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64)> {
    let durations: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.total_cold_start_duration)
        .collect();
    if durations.is_empty() {
        return None;
    }
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50))
}
