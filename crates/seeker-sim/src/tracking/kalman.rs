//! Constant-velocity Kalman filter on image-plane target center `(x, y)`.
//!
//! State vector: `[x, y, vx, vy]`. Measurements are noisy `(x, y)` centroids
//! from detections or motion blobs.

/// 4-state constant-velocity Kalman filter for a 2D point target.
///
/// # C# analogy
/// A small state-estimator class you'd unit-test in isolation before plugging
/// into a tracking pipeline — like a `KalmanFilter2d` service with `Predict` /
/// `Update` methods.
#[derive(Debug, Clone)]
pub struct KalmanFilter2d {
    /// State `[x, y, vx, vy]` in pixels and pixels/second.
    state: [f32; 4],
    /// Error covariance (4×4, row-major).
    covariance: [[f32; 4]; 4],
    /// Diagonal process noise scale (tuned for small moving targets).
    process_noise: f32,
    /// Per-axis measurement noise variance (pixels²).
    measurement_noise: f32,
}

impl KalmanFilter2d {
    /// Creates a filter initialized at `(x, y)` with zero velocity.
    ///
    /// # Arguments
    /// * `x` — initial horizontal position (pixels).
    /// * `y` — initial vertical position (pixels).
    ///
    /// # Returns
    /// Filter ready for [`Self::predict`] / [`Self::update`] calls.
    pub fn new(x: f32, y: f32) -> Self {
        Self {
            state: [x, y, 0.0, 0.0],
            covariance: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 100.0, 0.0],
                [0.0, 0.0, 0.0, 100.0],
            ],
            process_noise: 1.0,
            measurement_noise: 4.0,
        }
    }

    /// Advances the state by `dt` seconds (prediction step).
    ///
    /// # Arguments
    /// * `dt` — elapsed time since the last predict or update (seconds).
    pub fn predict(&mut self, dt: f32) {
        let f = transition_matrix(dt);
        self.state = mat4_vec4_mul(&f, &self.state);
        let fp = mat4_mul(&f, &self.covariance);
        let fpft = mat4_mul(&fp, &transpose4(&f));
        self.covariance = mat4_add(&fpft, &process_noise_matrix(self.process_noise, dt));
    }

    /// Fuses a position measurement `(mx, my)` (update step).
    ///
    /// # Arguments
    /// * `mx` — measured x (pixels).
    /// * `my` — measured y (pixels).
    pub fn update(&mut self, mx: f32, my: f32) {
        // Innovation y = z - H x  (H selects position)
        let innovation = [
            mx - self.state[0],
            my - self.state[1],
        ];

        // S = H P H^T + R  (2×2)
        let p = &self.covariance;
        let r = self.measurement_noise;
        let s00 = p[0][0] + r;
        let s01 = p[0][1];
        let s10 = p[1][0];
        let s11 = p[1][1] + r;

        let det = s00 * s11 - s01 * s10;
        if det.abs() < 1e-12 {
            return;
        }

        // K = P H^T S^{-1  (4×2)
        let inv_det = 1.0 / det;
        let si00 = s11 * inv_det;
        let si01 = -s01 * inv_det;
        let si10 = -s10 * inv_det;
        let si11 = s00 * inv_det;

        let ph_t = [
            [p[0][0], p[0][1]],
            [p[1][0], p[1][1]],
            [p[2][0], p[2][1]],
            [p[3][0], p[3][1]],
        ];

        let mut k = [[0.0_f32; 2]; 4];
        for i in 0..4 {
            k[i][0] = ph_t[i][0] * si00 + ph_t[i][1] * si10;
            k[i][1] = ph_t[i][0] * si01 + ph_t[i][1] * si11;
        }

        // x = x + K y
        for i in 0..4 {
            self.state[i] += k[i][0] * innovation[0] + k[i][1] * innovation[1];
        }

        // P = (I - K H) P
        let mut kh = [[0.0_f32; 4]; 4];
        for i in 0..4 {
            kh[i][0] = k[i][0];
            kh[i][1] = k[i][1];
        }

        let i_minus_kh = [
            [1.0 - kh[0][0], -kh[0][1], 0.0, 0.0],
            [-kh[1][0], 1.0 - kh[1][1], 0.0, 0.0],
            [-kh[2][0], -kh[2][1], 1.0, 0.0],
            [-kh[3][0], -kh[3][1], 0.0, 1.0],
        ];

        self.covariance = mat4_mul(&i_minus_kh, &self.covariance);
    }

    /// Filtered position `(x, y)` in pixels.
    pub fn position(&self) -> (f32, f32) {
        (self.state[0], self.state[1])
    }

    /// Filtered velocity `(vx, vy)` in pixels per second.
    pub fn velocity(&self) -> (f32, f32) {
        (self.state[2], self.state[3])
    }
}

// --- small linear-algebra helpers (private) ---

fn transition_matrix(dt: f32) -> [[f32; 4]; 4] {
    [
        [1.0, 0.0, dt, 0.0],
        [0.0, 1.0, 0.0, dt],
        [0.0, 0.0, 1.0, 0.0],
        [0.0, 0.0, 0.0, 1.0],
    ]
}

fn process_noise_matrix(q: f32, dt: f32) -> [[f32; 4]; 4] {
    let dt2 = dt * dt;
    let dt3 = dt2 * dt;
    let dt4 = dt2 * dt2;
    let q11 = q * dt4 / 4.0;
    let q13 = q * dt3 / 2.0;
    let q33 = q * dt2;
    [
        [q11, 0.0, q13, 0.0],
        [0.0, q11, 0.0, q13],
        [q13, 0.0, q33, 0.0],
        [0.0, q13, 0.0, q33],
    ]
}

fn transpose4(m: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut t = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            t[i][j] = m[j][i];
        }
    }
    t
}

fn mat4_mul(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = (0..4).map(|k| a[i][k] * b[k][j]).sum();
        }
    }
    out
}

fn mat4_add(a: &[[f32; 4]; 4], b: &[[f32; 4]; 4]) -> [[f32; 4]; 4] {
    let mut out = [[0.0; 4]; 4];
    for i in 0..4 {
        for j in 0..4 {
            out[i][j] = a[i][j] + b[i][j];
        }
    }
    out
}

fn mat4_vec4_mul(m: &[[f32; 4]; 4], v: &[f32; 4]) -> [f32; 4] {
    let mut out = [0.0; 4];
    for i in 0..4 {
        out[i] = (0..4).map(|j| m[i][j] * v[j]).sum();
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn predict_moves_position_by_velocity_times_dt() {
        let mut kf = KalmanFilter2d::new(100.0, 200.0);
        kf.state[2] = 30.0;
        kf.state[3] = -10.0;

        kf.predict(1.0);

        let (x, y) = kf.position();
        assert!((x - 130.0).abs() < 0.01, "x expected 130, got {x}");
        assert!((y - 190.0).abs() < 0.01, "y expected 190, got {y}");
    }

    #[test]
    fn constant_velocity_measurements_converge_to_true_velocity() {
        let dt = 1.0 / 30.0;
        let true_vx = 120.0_f32;
        let true_vy = 40.0_f32;

        let mut kf = KalmanFilter2d::new(0.0, 0.0);

        for frame in 0..60 {
            let t = frame as f32 * dt;
            let mx = true_vx * t;
            let my = true_vy * t;
            kf.predict(dt);
            kf.update(mx, my);
        }

        let (vx, vy) = kf.velocity();
        assert!(
            (vx - true_vx).abs() < 15.0,
            "vx expected ~{true_vx}, got {vx}"
        );
        assert!(
            (vy - true_vy).abs() < 15.0,
            "vy expected ~{true_vy}, got {vy}"
        );

        let (x, y) = kf.position();
        let t_end = 59.0 * dt;
        assert!((x - true_vx * t_end).abs() < 5.0);
        assert!((y - true_vy * t_end).abs() < 5.0);
    }

    #[test]
    fn update_pulls_position_toward_measurement() {
        let mut kf = KalmanFilter2d::new(0.0, 0.0);
        kf.update(50.0, 25.0);

        let (x, y) = kf.position();
        assert!(x > 0.0 && x <= 50.0);
        assert!(y > 0.0 && y <= 25.0);
    }
}
