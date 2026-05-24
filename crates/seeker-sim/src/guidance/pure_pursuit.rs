//! Pure pursuit — baseline guidance that steers toward the current line-of-sight.
//!
//! Teaching baseline (ADR-006): turn command from **bearing error** `λ`, not bearing
//! rate `λ̇` (which PN uses).
//!
//! ```text
//! a_cmd = N * V_c * λ
//! ```
//!
//! Sign convention matches [`crate::tracking::los::line_of_sight`] and [`super::pn`].

/// Commanded lateral acceleration from pure pursuit.
///
/// # Arguments
/// * `navigation_constant` — gain `N` (reuse PN config default 3.0).
/// * `closing_velocity` — speed scale `V_c` (sim units).
/// * `los_rad` — current line-of-sight angle `λ` (radians) from the seeker reference.
///
/// # Returns
/// Lateral acceleration command (same units as [`super::pn::proportional_navigation`]).
///
/// # C# analogy
/// `Guidance.PurePursuit(N, speed, bearingError)` — steer toward where the target is *now*.
pub fn pure_pursuit(
    navigation_constant: f32,
    closing_velocity: f32,
    los_rad: f32,
) -> f32 {
    navigation_constant * closing_velocity * los_rad
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: f32 = 3.0;
    const V_C: f32 = 100.0;

    #[test]
    fn zero_los_yields_zero_accel() {
        assert!(pure_pursuit(N, V_C, 0.0).abs() < 1e-6);
    }

    #[test]
    fn positive_los_yields_positive_accel() {
        let a = pure_pursuit(N, V_C, 0.05);
        assert!(a > 0.0);
        assert!((a - N * V_C * 0.05).abs() < 1e-4);
    }

    #[test]
    fn negative_los_yields_negative_accel() {
        assert!(pure_pursuit(N, V_C, -0.03) < 0.0);
    }
}
