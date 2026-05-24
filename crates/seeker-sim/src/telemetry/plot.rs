//! PNG plots for intercept runs (Phase 5D) — `trajectory.png` via `plotters`.

use crate::telemetry::record::{GuidanceRecord, SimRecord};
use crate::telemetry::writer::{PlotRenderError, TelemetryError};
use plotters::prelude::*;
use std::path::Path;

const PNG_WIDTH: u32 = 960;
const PNG_HEIGHT: u32 = 720;

/// Renders `trajectory.png`: top = 2D paths; bottom-left = miss distance; bottom-right = LOS rate.
///
/// Uses [`LineSeries`] only (no point markers) and **f64** chart coordinates throughout
/// so plotters does not mix `f32`/`i32` coordinate types.
///
/// # C# analogy
/// Exporting a multi-panel figure to PNG without a GUI — like ScottPlot `SavePng`.
pub fn write_trajectory_png(
    path: &Path,
    sim: &[SimRecord],
    guidance: &[GuidanceRecord],
) -> Result<(), TelemetryError> {
    if sim.is_empty() {
        return Err(TelemetryError::EmptyPlotData);
    }

    let root = BitMapBackend::new(path, (PNG_WIDTH, PNG_HEIGHT)).into_drawing_area();
    root.fill(&WHITE).map_err(|e| plot_err(path, e))?;

    let (top, bottom) = root.split_vertically((55).percent());
    let (miss_area, los_area) = bottom.split_horizontally((50).percent());

    // --- Top: 2D trajectory ---
    {
        let (min_x, max_x, min_y, max_y) = sim_xy_bounds(sim);
        let mut chart = ChartBuilder::on(&top)
            .caption("2D trajectory (sim units)", ("sans-serif", 22))
            .margin(10)
            .x_label_area_size(32)
            .y_label_area_size(42)
            .build_cartesian_2d(min_x..max_x, min_y..max_y)
            .map_err(|e| plot_err(path, e))?;

        chart
            .configure_mesh()
            .x_desc("x")
            .y_desc("y")
            .draw()
            .map_err(|e| plot_err(path, e))?;

        let interceptor: Vec<(f64, f64)> = sim
            .iter()
            .map(|r| (f64::from(r.interceptor_x), f64::from(r.interceptor_y)))
            .collect();
        let target: Vec<(f64, f64)> = sim
            .iter()
            .map(|r| (f64::from(r.target_x), f64::from(r.target_y)))
            .collect();

        chart
            .draw_series(LineSeries::new(interceptor, BLUE.stroke_width(2)))
            .map_err(|e| plot_err(path, e))?;
        chart
            .draw_series(LineSeries::new(target, RED.stroke_width(2)))
            .map_err(|e| plot_err(path, e))?;
    }

    // --- Bottom-left: miss distance vs time ---
    {
        let t_min = sim.first().map(|r| r.time_s).unwrap_or(0.0);
        let t_max = sim.last().map(|r| r.time_s).unwrap_or(1.0);
        let miss_min = sim
            .iter()
            .map(|r| f64::from(r.miss_distance))
            .fold(f64::MAX, f64::min);
        let miss_max = sim
            .iter()
            .map(|r| f64::from(r.miss_distance))
            .fold(f64::MIN, f64::max);
        let miss_pad = ((miss_max - miss_min) * 0.1).max(5.0);

        let mut chart = ChartBuilder::on(&miss_area)
            .caption("Miss distance", ("sans-serif", 16))
            .margin(8)
            .x_label_area_size(28)
            .y_label_area_size(42)
            .build_cartesian_2d(t_min..t_max, (miss_min - miss_pad)..(miss_max + miss_pad))
            .map_err(|e| plot_err(path, e))?;

        chart
            .configure_mesh()
            .x_desc("time (s)")
            .y_desc("miss")
            .draw()
            .map_err(|e| plot_err(path, e))?;

        let series: Vec<(f64, f64)> = sim
            .iter()
            .map(|r| (r.time_s, f64::from(r.miss_distance)))
            .collect();
        chart
            .draw_series(LineSeries::new(series, GREEN.stroke_width(2)))
            .map_err(|e| plot_err(path, e))?;
    }

    // --- Bottom-right: LOS rate vs time ---
    if !guidance.is_empty() {
        let t_min = sim.first().map(|r| r.time_s).unwrap_or(0.0);
        let t_max = sim.last().map(|r| r.time_s).unwrap_or(1.0);

        let los_rates: Vec<(f64, f64)> = guidance
            .iter()
            .map(|g| {
                let t = sim
                    .iter()
                    .find(|s| s.frame_index == g.frame_index)
                    .map(|s| s.time_s)
                    .unwrap_or(0.0);
                (t, f64::from(g.los_rate))
            })
            .collect();

        let los_abs = los_rates
            .iter()
            .map(|(_, r)| r.abs())
            .fold(0.01_f64, f64::max);
        let los_min = -los_abs * 1.2;
        let los_max = los_abs * 1.2;

        let mut chart = ChartBuilder::on(&los_area)
            .caption("LOS rate", ("sans-serif", 16))
            .margin(8)
            .x_label_area_size(28)
            .y_label_area_size(42)
            .build_cartesian_2d(t_min..t_max, los_min..los_max)
            .map_err(|e| plot_err(path, e))?;

        chart
            .configure_mesh()
            .x_desc("time (s)")
            .y_desc("rad/s")
            .draw()
            .map_err(|e| plot_err(path, e))?;

        chart
            .draw_series(LineSeries::new(los_rates, MAGENTA.stroke_width(2)))
            .map_err(|e| plot_err(path, e))?;
    }

    root.present().map_err(|e| plot_err(path, e))?;
    Ok(())
}

fn plot_err(path: &Path, err: impl std::fmt::Display) -> TelemetryError {
    TelemetryError::Plot {
        path: path.to_path_buf(),
        source: PlotRenderError::new(err.to_string()),
    }
}

fn sim_xy_bounds(sim: &[SimRecord]) -> (f64, f64, f64, f64) {
    let mut min_x = f64::MAX;
    let mut max_x = f64::MIN;
    let mut min_y = f64::MAX;
    let mut max_y = f64::MIN;

    for r in sim {
        for (x, y) in [
            (r.interceptor_x, r.interceptor_y),
            (r.target_x, r.target_y),
        ] {
            let x = f64::from(x);
            let y = f64::from(y);
            min_x = min_x.min(x);
            max_x = max_x.max(x);
            min_y = min_y.min(y);
            max_y = max_y.max(y);
        }
    }

    let span = (max_x - min_x).max(max_y - min_y).max(1.0);
    let pad = span * 0.08;
    (min_x - pad, max_x + pad, min_y - pad, max_y + pad)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_png() -> (std::path::PathBuf, std::path::PathBuf) {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("seeker_plot_{nanos}"));
        fs::create_dir_all(&dir).unwrap();
        (dir.join("trajectory.png"), dir)
    }

    fn sample_sim() -> Vec<SimRecord> {
        (0..10)
            .map(|i| {
                let t = i as f64 * 0.033;
                let x = 500.0 - i as f32 * 40.0;
                SimRecord {
                    frame_index: i as u64 + 1,
                    time_s: t,
                    interceptor_x: x - 200.0,
                    interceptor_y: i as f32 * 2.0,
                    target_x: x,
                    target_y: 100.0 + i as f32,
                    interceptor_vx: 120.0,
                    interceptor_vy: 1.0,
                    miss_distance: 200.0 - i as f32 * 5.0,
                }
            })
            .collect()
    }

    fn sample_guidance(count: usize) -> Vec<GuidanceRecord> {
        (0..count)
            .map(|i| GuidanceRecord {
                frame_index: i as u64 + 1,
                track_id: 1,
                los: 0.1,
                los_rate: -0.02 + i as f32 * 0.001,
                law: "pn".into(),
                commanded_lateral_accel: 3.0,
            })
            .collect()
    }

    #[test]
    fn writes_non_empty_png() {
        let (path, dir) = temp_png();
        let sim = sample_sim();
        let guidance = sample_guidance(sim.len());

        write_trajectory_png(&path, &sim, &guidance).expect("plot");

        let meta = fs::metadata(&path).expect("png exists");
        assert!(meta.len() > 500, "png too small: {} bytes", meta.len());

        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn empty_sim_returns_error() {
        let (path, dir) = temp_png();
        let err = write_trajectory_png(&path, &[], &[]).unwrap_err();
        assert!(matches!(err, TelemetryError::EmptyPlotData));
        let _ = fs::remove_dir_all(&dir);
    }
}
