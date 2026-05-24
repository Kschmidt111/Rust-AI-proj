//! Proportional navigation (PN) — commanded lateral acceleration from LOS rate.
//!
//! Classic teaching form (see ADR-006, ARCHITECTURE §4.5):
//!
//! ```text
//! a_cmd = N * V_c * λ̇
//! ```
//!
//! where `N` is the navigation constant (typically 3–5), `V_c` is closing velocity
//! (sim units), and `λ̇` is line-of-sight rate in rad/s from [`tracking::los`].
//!
//! Sign convention matches [`tracking::los::line_of_sight`]: positive `λ̇` means the
//! target bearing is increasing → positive `a_cmd` turns the interceptor **clockwise**
//! in sim space (see `sim::engine` right-hand normal).

/// Commanded lateral acceleration from proportional navigation.
///
/// # Arguments
/// * `navigation_constant` — PN gain `N` (dimensionless; config default 3.0).
/// * `closing_velocity` — magnitude of closing velocity `V_c` (sim units, e.g. m/s).
/// * `los_rate_rad_per_s` — line-of-sight rate `λ̇` (rad/s).
///
/// # Returns
/// Commanded lateral acceleration in the same units as `V_c`² scaling (sim m/s² when
/// `V_c` is m/s and `λ̇` is rad/s).
///
/// # C# analogy
/// A static helper like `Guidance.Pn(N, closingSpeed, losRate)` returning a scalar
/// command — no object state, easy to unit-test.
pub fn proportional_navigation(
    navigation_constant: f32,
    closing_velocity: f32,
    los_rate_rad_per_s: f32,
) -> f32 {
    navigation_constant * closing_velocity * los_rate_rad_per_s
}

#[cfg(test)]
mod tests {
    use super::*;

    const N: f32 = 3.0;
    const V_C: f32 = 100.0;

    #[test]
    fn zero_los_rate_yields_zero_accel() {
        let a = proportional_navigation(N, V_C, 0.0);
        assert!(a.abs() < 1e-6);
    }

    #[test]
    fn constant_positive_los_rate_yields_positive_accel() {
        let los_rate = 0.05_f32; // rad/s
        let a = proportional_navigation(N, V_C, los_rate);
        assert!(a > 0.0, "expected positive accel for positive los_rate, got {a}");
        assert!((a - N * V_C * los_rate).abs() < 1e-4);
    }

    #[test]
    fn negative_los_rate_yields_negative_accel() {
        let a = proportional_navigation(N, V_C, -0.02);
        assert!(a < 0.0);
    }

    #[test]
    fn scales_linearly_with_navigation_constant() {
        let los_rate = 0.01;
        let a3 = proportional_navigation(3.0, V_C, los_rate);
        let a6 = proportional_navigation(6.0, V_C, los_rate);
        assert!((a6 - 2.0 * a3).abs() < 1e-4);
    }

    #[test]
    fn scales_linearly_with_closing_velocity() {
        let los_rate = 0.01;
        let a100 = proportional_navigation(N, 100.0, los_rate);
        let a200 = proportional_navigation(N, 200.0, los_rate);
        assert!((a200 - 2.0 * a100).abs() < 1e-4);
    }
}
