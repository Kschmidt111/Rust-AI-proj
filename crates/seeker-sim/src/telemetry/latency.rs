//! Per-frame latency summaries for CLI / demo output (Phase 6F).

/// p50 / p95 / max frame processing times in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FrameLatencySummary {
    pub count: usize,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub max_ms: f64,
}

/// Summarizes per-frame elapsed times (e.g. from pipeline frame stats).
///
/// # Arguments
/// * `elapsed_ms` — one duration per processed frame.
///
/// # Returns
/// `None` when `elapsed_ms` is empty.
pub fn summarize_frame_latency(elapsed_ms: &[f64]) -> Option<FrameLatencySummary> {
    if elapsed_ms.is_empty() {
        return None;
    }

    let mut sorted: Vec<f64> = elapsed_ms
        .iter()
        .copied()
        .filter(|v| v.is_finite())
        .collect();
    if sorted.is_empty() {
        return None;
    }
    sorted.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    Some(FrameLatencySummary {
        count: sorted.len(),
        p50_ms: percentile_sorted(&sorted, 0.50),
        p95_ms: percentile_sorted(&sorted, 0.95),
        max_ms: sorted[sorted.len() - 1],
    })
}

/// Percentile on a **sorted** slice (`p` in `0.0..=1.0`).
fn percentile_sorted(sorted: &[f64], p: f64) -> f64 {
    assert!(!sorted.is_empty());
    let p = p.clamp(0.0, 1.0);
    let idx = ((sorted.len() - 1) as f64 * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn summarize_empty_returns_none() {
        assert!(summarize_frame_latency(&[]).is_none());
    }

    #[test]
    fn p50_and_p95_on_known_samples() {
        let samples: Vec<f64> = (1..=100).map(|i| i as f64).collect();
        let s = summarize_frame_latency(&samples).expect("summary");
        assert_eq!(s.count, 100);
        assert!((s.p50_ms - 51.0).abs() < 1.0);
        assert!((s.p95_ms - 95.0).abs() < 1.0);
        assert!((s.max_ms - 100.0).abs() < 1e-6);
    }
}
