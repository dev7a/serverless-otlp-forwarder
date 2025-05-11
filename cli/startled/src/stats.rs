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
) -> Option<(f64, f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = cold_starts.iter().map(|m| m.init_duration).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start server duration
pub fn calculate_cold_start_server_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = cold_starts.iter().map(|m| m.duration).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for warm start metrics
pub fn calculate_warm_start_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
    field: fn(&crate::types::WarmStartMetrics) -> f64,
) -> Option<(f64, f64, f64, f64, f64)> {
    if warm_starts.is_empty() {
        return None;
    }
    let durations: Vec<f64> = warm_starts.iter().map(field).collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for client metrics
pub fn calculate_client_stats(
    client_measurements: &[crate::types::ClientMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if client_measurements.is_empty() {
        return None;
    }
    let durations: Vec<f64> = client_measurements
        .iter()
        .map(|m| m.client_duration)
        .collect();
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate memory usage statistics
pub fn calculate_memory_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if warm_starts.is_empty() {
        return None;
    }
    let memory: Vec<f64> = warm_starts
        .iter()
        .map(|m| m.max_memory_used as f64)
        .collect();
    let stats = calculate_stats(&memory);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start extension overhead
pub fn calculate_cold_start_extension_overhead_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    if cold_starts.is_empty() {
        return None;
    }
    let overheads: Vec<f64> = cold_starts.iter().map(|m| m.extension_overhead).collect();
    let stats = calculate_stats(&overheads);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start total cold start duration
pub fn calculate_cold_start_total_duration_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let durations: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.total_cold_start_duration)
        .collect();
    if durations.is_empty() {
        return None;
    }
    let stats = calculate_stats(&durations);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start response latency
pub fn calculate_cold_start_response_latency_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.response_latency_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start response duration
pub fn calculate_cold_start_response_duration_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.response_duration_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start runtime overhead
pub fn calculate_cold_start_runtime_overhead_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.runtime_overhead_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start produced bytes
pub fn calculate_cold_start_produced_bytes_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.produced_bytes.map(|b| b as f64))
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for cold start runtime done metrics duration
pub fn calculate_cold_start_runtime_done_metrics_duration_stats(
    cold_starts: &[crate::types::ColdStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = cold_starts
        .iter()
        .filter_map(|m| m.runtime_done_metrics_duration_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for warm start response latency
pub fn calculate_warm_start_response_latency_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = warm_starts
        .iter()
        .filter_map(|m| m.response_latency_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for warm start response duration
pub fn calculate_warm_start_response_duration_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = warm_starts
        .iter()
        .filter_map(|m| m.response_duration_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for warm start runtime overhead
pub fn calculate_warm_start_runtime_overhead_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = warm_starts
        .iter()
        .filter_map(|m| m.runtime_overhead_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for warm start produced bytes
pub fn calculate_warm_start_produced_bytes_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = warm_starts
        .iter()
        .filter_map(|m| m.produced_bytes.map(|b| b as f64))
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

/// Calculate statistics for warm start runtime done metrics duration
pub fn calculate_warm_start_runtime_done_metrics_duration_stats(
    warm_starts: &[crate::types::WarmStartMetrics],
) -> Option<(f64, f64, f64, f64, f64)> {
    let values: Vec<f64> = warm_starts
        .iter()
        .filter_map(|m| m.runtime_done_metrics_duration_ms)
        .collect();
    if values.is_empty() {
        return None;
    }
    let stats = calculate_stats(&values);
    Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{ClientMetrics, ColdStartMetrics, WarmStartMetrics};

    const EPSILON: f64 = 1e-9;

    fn assert_f64_eq(a: f64, b: f64, msg: &str) {
        assert!((a - b).abs() < EPSILON, "{} | {} vs {}", msg, a, b);
    }

    fn assert_metrics_stats_eq(actual: &MetricsStats, expected: &MetricsStats, context: &str) {
        assert_f64_eq(
            actual.mean,
            expected.mean,
            &format!("{}: mean mismatch", context),
        );
        assert_f64_eq(
            actual.p50,
            expected.p50,
            &format!("{}: p50 mismatch", context),
        );
        assert_f64_eq(
            actual.p95,
            expected.p95,
            &format!("{}: p95 mismatch", context),
        );
        assert_f64_eq(
            actual.p99,
            expected.p99,
            &format!("{}: p99 mismatch", context),
        );
        assert_f64_eq(
            actual.std_dev,
            expected.std_dev,
            &format!("{}: std_dev mismatch", context),
        );
    }

    fn assert_option_tuple_eq(
        actual: Option<(f64, f64, f64, f64, f64)>,
        expected: Option<(f64, f64, f64, f64, f64)>,
        context: &str,
    ) {
        match (actual, expected) {
            (Some(a), Some(e)) => {
                assert_f64_eq(a.0, e.0, &format!("{}: mean (tuple.0) mismatch", context));
                assert_f64_eq(a.1, e.1, &format!("{}: p99 (tuple.1) mismatch", context));
                assert_f64_eq(a.2, e.2, &format!("{}: p95 (tuple.2) mismatch", context));
                assert_f64_eq(a.3, e.3, &format!("{}: p50 (tuple.3) mismatch", context));
                assert_f64_eq(
                    a.4,
                    e.4,
                    &format!("{}: std_dev (tuple.4) mismatch", context),
                );
            }
            (None, None) => {} // Both are None, which is fine.
            _ => panic!(
                "{}: Option mismatch. Actual: {:?}, Expected: {:?}",
                context, actual, expected
            ),
        }
    }

    #[test]
    fn test_calculate_stats_empty_slice() {
        let values: [f64; 0] = [];
        let stats = calculate_stats(&values);
        let expected = MetricsStats {
            mean: 0.0,
            p50: 0.0,
            p95: 0.0,
            p99: 0.0,
            std_dev: 0.0,
        };
        assert_metrics_stats_eq(&stats, &expected, "empty_slice");
    }

    #[test]
    fn test_calculate_stats_single_value() {
        let values = [5.0];
        let stats = calculate_stats(&values);
        let expected = MetricsStats {
            mean: 5.0,
            p50: 5.0,
            p95: 5.0,
            p99: 5.0,
            std_dev: 0.0,
        };
        assert_metrics_stats_eq(&stats, &expected, "single_value");
    }

    #[test]
    fn test_calculate_stats_multiple_values() {
        let values = [1.0, 2.0, 3.0, 4.0, 5.0, 6.0, 7.0, 8.0, 9.0, 10.0, 100.0]; // N=11
        let stats = calculate_stats(&values);
        let mut data = statrs::statistics::Data::new(values.to_vec());
        let expected = MetricsStats {
            mean: data.mean().unwrap(),
            p50: data.percentile(50),
            p95: data.percentile(95),
            p99: data.percentile(99),
            std_dev: data.std_dev().unwrap(),
        };
        assert_metrics_stats_eq(&stats, &expected, "multiple_values");
    }

    #[test]
    fn test_calculate_cold_start_init_stats_empty() {
        let cold_starts: [ColdStartMetrics; 0] = [];
        let result = calculate_cold_start_init_stats(&cold_starts);
        assert_eq!(result, None, "cs_init_empty");
    }

    #[test]
    fn test_calculate_cold_start_init_stats_happy_path() {
        let cold_starts = [
            ColdStartMetrics {
                timestamp: "ts1".to_string(),
                init_duration: 100.0,
                duration: 200.0,
                extension_overhead: 10.0,
                total_cold_start_duration: Some(310.0),
                billed_duration: 300,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            ColdStartMetrics {
                timestamp: "ts2".to_string(),
                init_duration: 120.0,
                duration: 220.0,
                extension_overhead: 12.0,
                total_cold_start_duration: Some(352.0),
                billed_duration: 320,
                max_memory_used: 130,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_cold_start_init_stats(&cold_starts);
        let durations = [100.0, 120.0];
        let stats = calculate_stats(&durations);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "cs_init_happy");
    }

    #[test]
    fn test_calculate_cold_start_server_stats_empty() {
        let cold_starts: [ColdStartMetrics; 0] = [];
        let result = calculate_cold_start_server_stats(&cold_starts);
        assert_eq!(result, None, "cs_server_empty");
    }

    #[test]
    fn test_calculate_cold_start_server_stats_happy_path() {
        let cold_starts = [
            ColdStartMetrics {
                timestamp: "ts1".to_string(),
                init_duration: 100.0,
                duration: 200.0,
                extension_overhead: 10.0,
                total_cold_start_duration: Some(310.0),
                billed_duration: 300,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            ColdStartMetrics {
                timestamp: "ts2".to_string(),
                init_duration: 120.0,
                duration: 220.0,
                extension_overhead: 12.0,
                total_cold_start_duration: Some(352.0),
                billed_duration: 320,
                max_memory_used: 130,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_cold_start_server_stats(&cold_starts);
        let durations = [200.0, 220.0];
        let stats = calculate_stats(&durations);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "cs_server_happy");
    }

    fn get_warm_duration(ws: &WarmStartMetrics) -> f64 {
        ws.duration
    }

    #[test]
    fn test_calculate_warm_start_stats_empty() {
        let warm_starts: [WarmStartMetrics; 0] = [];
        let result = calculate_warm_start_stats(&warm_starts, get_warm_duration);
        assert_eq!(result, None, "ws_empty");
    }

    #[test]
    fn test_calculate_warm_start_stats_happy_path() {
        let warm_starts = [
            WarmStartMetrics {
                timestamp: "ts1".to_string(),
                duration: 50.0,
                extension_overhead: 5.0,
                billed_duration: 50,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            WarmStartMetrics {
                timestamp: "ts2".to_string(),
                duration: 60.0,
                extension_overhead: 6.0,
                billed_duration: 60,
                max_memory_used: 130,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_warm_start_stats(&warm_starts, get_warm_duration);
        let durations = [50.0, 60.0];
        let stats = calculate_stats(&durations);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "ws_happy");
    }

    #[test]
    fn test_calculate_client_stats_empty() {
        let client_metrics: [ClientMetrics; 0] = [];
        let result = calculate_client_stats(&client_metrics);
        assert_eq!(result, None, "client_empty");
    }

    #[test]
    fn test_calculate_client_stats_happy_path() {
        let client_metrics = [
            ClientMetrics {
                timestamp: "ts1".to_string(),
                client_duration: 30.0,
                memory_size: 256,
            },
            ClientMetrics {
                timestamp: "ts2".to_string(),
                client_duration: 35.0,
                memory_size: 256,
            },
        ];
        let result = calculate_client_stats(&client_metrics);
        let durations = [30.0, 35.0];
        let stats = calculate_stats(&durations);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "client_happy");
    }

    #[test]
    fn test_calculate_memory_stats_empty() {
        let warm_starts: [WarmStartMetrics; 0] = [];
        let result = calculate_memory_stats(&warm_starts);
        assert_eq!(result, None, "mem_empty");
    }

    #[test]
    fn test_calculate_memory_stats_happy_path() {
        let warm_starts = [
            WarmStartMetrics {
                timestamp: "ts1".to_string(),
                duration: 50.0,
                extension_overhead: 5.0,
                billed_duration: 50,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            WarmStartMetrics {
                timestamp: "ts2".to_string(),
                duration: 60.0,
                extension_overhead: 6.0,
                billed_duration: 60,
                max_memory_used: 256,
                memory_size: 512,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_memory_stats(&warm_starts);
        let memory_values = [128.0, 256.0];
        let stats = calculate_stats(&memory_values);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "mem_happy");
    }

    #[test]
    fn test_calculate_cold_start_extension_overhead_stats_empty() {
        let cold_starts: [ColdStartMetrics; 0] = [];
        let result = calculate_cold_start_extension_overhead_stats(&cold_starts);
        assert_eq!(result, None, "cs_ext_overhead_empty");
    }

    #[test]
    fn test_calculate_cold_start_extension_overhead_stats_happy_path() {
        let cold_starts = [
            ColdStartMetrics {
                timestamp: "ts1".to_string(),
                init_duration: 100.0,
                duration: 200.0,
                extension_overhead: 10.0,
                total_cold_start_duration: Some(310.0),
                billed_duration: 300,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            ColdStartMetrics {
                timestamp: "ts2".to_string(),
                init_duration: 120.0,
                duration: 220.0,
                extension_overhead: 12.0,
                total_cold_start_duration: Some(352.0),
                billed_duration: 320,
                max_memory_used: 130,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_cold_start_extension_overhead_stats(&cold_starts);
        let overheads = [10.0, 12.0];
        let stats = calculate_stats(&overheads);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "cs_ext_overhead_happy");
    }

    #[test]
    fn test_calculate_cold_start_total_duration_stats_empty_input() {
        let cold_starts: [ColdStartMetrics; 0] = [];
        let result = calculate_cold_start_total_duration_stats(&cold_starts);
        assert_eq!(result, None, "cs_total_dur_empty_input");
    }

    #[test]
    fn test_calculate_cold_start_total_duration_stats_all_none() {
        let cold_starts = [
            ColdStartMetrics {
                timestamp: "ts1".to_string(),
                init_duration: 100.0,
                duration: 200.0,
                extension_overhead: 10.0,
                total_cold_start_duration: None,
                billed_duration: 300,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            ColdStartMetrics {
                timestamp: "ts2".to_string(),
                init_duration: 120.0,
                duration: 220.0,
                extension_overhead: 12.0,
                total_cold_start_duration: None,
                billed_duration: 320,
                max_memory_used: 130,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_cold_start_total_duration_stats(&cold_starts);
        assert_eq!(result, None, "cs_total_dur_all_none");
    }

    #[test]
    fn test_calculate_cold_start_total_duration_stats_happy_path() {
        let cold_starts = [
            ColdStartMetrics {
                timestamp: "ts1".to_string(),
                init_duration: 100.0,
                duration: 200.0,
                extension_overhead: 10.0,
                total_cold_start_duration: Some(310.0),
                billed_duration: 300,
                max_memory_used: 128,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
            ColdStartMetrics {
                timestamp: "ts2".to_string(),
                init_duration: 120.0,
                duration: 220.0,
                extension_overhead: 12.0,
                total_cold_start_duration: None,
                billed_duration: 320,
                max_memory_used: 130,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            }, // One None
            ColdStartMetrics {
                timestamp: "ts3".to_string(),
                init_duration: 130.0,
                duration: 230.0,
                extension_overhead: 13.0,
                total_cold_start_duration: Some(373.0),
                billed_duration: 330,
                max_memory_used: 140,
                memory_size: 256,
                response_latency_ms: None,
                response_duration_ms: None,
                runtime_overhead_ms: None,
                produced_bytes: None,
                runtime_done_metrics_duration_ms: None,
            },
        ];
        let result = calculate_cold_start_total_duration_stats(&cold_starts);
        let durations = [310.0, 373.0]; // Only Some values
        let stats = calculate_stats(&durations);
        let expected = Some((stats.mean, stats.p99, stats.p95, stats.p50, stats.std_dev));
        assert_option_tuple_eq(result, expected, "cs_total_dur_happy");
    }
}
