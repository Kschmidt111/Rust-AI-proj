//! Guidance laws — steer the simulated interceptor from track geometry (Phase 5).
//!
//! Laws are **pure functions** (no hidden state). Pipeline / sim call them each step
//! with LOS rate and config gains.

pub mod pn;
pub mod pure_pursuit;

pub use pn::proportional_navigation;
pub use pure_pursuit::pure_pursuit;

use thiserror::Error;

/// Unknown `[guidance].law` value (expected `pn` or `pp`).
#[derive(Debug, Error, PartialEq, Eq)]
#[error("unknown guidance law '{law}' — expected 'pn' or 'pp'")]
pub struct UnknownGuidanceLaw {
    /// Value from config or API request.
    pub law: String,
}

/// Computes commanded lateral acceleration for PN or pure pursuit.
///
/// # Arguments
/// * `law` — `"pn"` or `"pp"` (case-insensitive).
/// * `n` — navigation constant.
/// * `v_c` — closing velocity (sim units).
/// * `los` — line-of-sight angle (radians).
/// * `los_rate` — LOS rate (rad/s); used by PN only.
///
/// # Returns
/// Lateral acceleration command in sim units (e.g. m/s²).
pub fn lateral_accel(
    law: &str,
    n: f32,
    v_c: f32,
    los: f32,
    los_rate: f32,
) -> Result<f32, UnknownGuidanceLaw> {
    if law.eq_ignore_ascii_case("pn") {
        Ok(proportional_navigation(n, v_c, los_rate))
    } else if law.eq_ignore_ascii_case("pp") {
        Ok(pure_pursuit(n, v_c, los))
    } else {
        Err(UnknownGuidanceLaw {
            law: law.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lateral_accel_pn_uses_los_rate_on_first_step() {
        let a = lateral_accel("pn", 3.0, 100.0, 0.1, 0.0).unwrap();
        assert!(a.abs() < 1e-6);
    }

    #[test]
    fn lateral_accel_rejects_unknown_law() {
        let err = lateral_accel("foo", 3.0, 100.0, 0.0, 0.0).unwrap_err();
        assert_eq!(err.law, "foo");
    }
}
