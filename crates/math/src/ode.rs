//! Ordinary Differential Equation (ODE) solvers:
//! classical Runge-Kutta 4th order (RK4) and adaptive Runge-Kutta 4(5) Dormand-Prince.

/// Classical 4th order Runge-Kutta (RK4) for autonomous or non-autonomous systems:
/// `dy/dt = f(t, y)`.
///
/// Parameters:
/// - `f`: derivative function `f(t: f64, y: &[f64], out_dy: &mut [f64])`
/// - `t0`: start time
/// - `y0`: initial state vector
/// - `t1`: end time
/// - `steps`: number of integration steps (`>= 1`)
///
/// Returns `(times, trajectories)`.
pub fn ode_rk4<F>(mut f: F, t0: f64, y0: &[f64], t1: f64, steps: usize) -> (Vec<f64>, Vec<Vec<f64>>)
where
    F: FnMut(f64, &[f64], &mut [f64]),
{
    let n = y0.len();
    let num_steps = if steps == 0 { 1 } else { steps };
    let dt = (t1 - t0) / num_steps as f64;

    let mut times = Vec::with_capacity(num_steps + 1);
    let mut traj = Vec::with_capacity(num_steps + 1);

    let mut t = t0;
    let mut y = y0.to_vec();

    times.push(t);
    traj.push(y.clone());

    let mut k1 = vec![0.0; n];
    let mut k2 = vec![0.0; n];
    let mut k3 = vec![0.0; n];
    let mut k4 = vec![0.0; n];
    let mut temp = vec![0.0; n];

    for _ in 0..num_steps {
        f(t, &y, &mut k1);

        for i in 0..n {
            temp[i] = y[i] + 0.5 * dt * k1[i];
        }
        f(t + 0.5 * dt, &temp, &mut k2);

        for i in 0..n {
            temp[i] = y[i] + 0.5 * dt * k2[i];
        }
        f(t + 0.5 * dt, &temp, &mut k3);

        for i in 0..n {
            temp[i] = y[i] + dt * k3[i];
        }
        f(t + dt, &temp, &mut k4);

        for i in 0..n {
            y[i] += (dt / 6.0) * (k1[i] + 2.0 * k2[i] + 2.0 * k3[i] + k4[i]);
        }
        t += dt;

        times.push(t);
        traj.push(y.clone());
    }

    (times, traj)
}

/// Adaptive Runge-Kutta Dormand-Prince (RK45) with automatic step-size control.
///
/// Parameters:
/// - `f`: derivative function `f(t: f64, y: &[f64], out_dy: &mut [f64])`
/// - `t0`: start time
/// - `y0`: initial state vector
/// - `t1`: end time
/// - `tol`: target error tolerance per step (e.g. `1e-6`)
pub fn ode_rk45<F>(mut f: F, t0: f64, y0: &[f64], t1: f64, tol: f64) -> (Vec<f64>, Vec<Vec<f64>>)
where
    F: FnMut(f64, &[f64], &mut [f64]),
{
    let n = y0.len();
    let mut times = Vec::new();
    let mut traj = Vec::new();

    let mut t = t0;
    let mut y = y0.to_vec();

    times.push(t);
    traj.push(y.clone());

    if n == 0 || (t1 - t0).abs() < 1e-15 {
        return (times, traj);
    }

    let tolerance = if tol <= 0.0 { 1e-6 } else { tol };
    let mut h = (t1 - t0) * 0.05;

    let mut k = [
        vec![0.0; n],
        vec![0.0; n],
        vec![0.0; n],
        vec![0.0; n],
        vec![0.0; n],
        vec![0.0; n],
        vec![0.0; n],
    ];
    let mut y_temp = vec![0.0; n];
    let mut y5 = vec![0.0; n];
    let mut y4 = vec![0.0; n];

    // Dormand-Prince coefficients
    const C: [f64; 7] = [0.0, 1.0 / 5.0, 3.0 / 10.0, 4.0 / 5.0, 8.0 / 9.0, 1.0, 1.0];
    const B5: [f64; 7] = [
        35.0 / 384.0,
        0.0,
        500.0 / 1113.0,
        125.0 / 192.0,
        -2187.0 / 6784.0,
        11.0 / 84.0,
        0.0,
    ];
    const B4: [f64; 7] = [
        5179.0 / 57600.0,
        0.0,
        7571.0 / 16695.0,
        393.0 / 640.0,
        -92097.0 / 339200.0,
        187.0 / 2100.0,
        1.0 / 40.0,
    ];

    while (t1 - t).abs() > 1e-12 {
        if (t + h - t1).abs() < 1e-12 || (h > 0.0 && t + h > t1) || (h < 0.0 && t + h < t1) {
            h = t1 - t;
        }

        // Stage 0
        f(t + C[0] * h, &y, &mut k[0]);

        // Stage 1
        for i in 0..n {
            y_temp[i] = y[i] + h * (1.0 / 5.0 * k[0][i]);
        }
        f(t + C[1] * h, &y_temp, &mut k[1]);

        // Stage 2
        for i in 0..n {
            y_temp[i] = y[i] + h * (3.0 / 40.0 * k[0][i] + 9.0 / 40.0 * k[1][i]);
        }
        f(t + C[2] * h, &y_temp, &mut k[2]);

        // Stage 3
        for i in 0..n {
            y_temp[i] = y[i] + h * (44.0 / 45.0 * k[0][i] - 56.0 / 15.0 * k[1][i] + 32.0 / 9.0 * k[2][i]);
        }
        f(t + C[3] * h, &y_temp, &mut k[3]);

        // Stage 4
        for i in 0..n {
            y_temp[i] = y[i] + h * (
                19372.0 / 6561.0 * k[0][i]
                - 25360.0 / 2187.0 * k[1][i]
                + 64448.0 / 6561.0 * k[2][i]
                - 212.0 / 729.0 * k[3][i]
            );
        }
        f(t + C[4] * h, &y_temp, &mut k[4]);

        // Stage 5
        for i in 0..n {
            y_temp[i] = y[i] + h * (
                9017.0 / 3168.0 * k[0][i]
                - 355.0 / 33.0 * k[1][i]
                + 46732.0 / 5247.0 * k[2][i]
                + 49.0 / 176.0 * k[3][i]
                - 5103.0 / 18656.0 * k[4][i]
            );
        }
        f(t + C[5] * h, &y_temp, &mut k[5]);

        // Stage 6
        for i in 0..n {
            y_temp[i] = y[i] + h * (
                35.0 / 384.0 * k[0][i]
                + 500.0 / 1113.0 * k[2][i]
                + 125.0 / 192.0 * k[3][i]
                - 2187.0 / 6784.0 * k[4][i]
                + 11.0 / 84.0 * k[5][i]
            );
        }
        f(t + C[6] * h, &y_temp, &mut k[6]);

        // 5th order and 4th order solutions
        for i in 0..n {
            let mut s5 = 0.0;
            let mut s4 = 0.0;
            for j in 0..7 {
                s5 += B5[j] * k[j][i];
                s4 += B4[j] * k[j][i];
            }
            y5[i] = y[i] + h * s5;
            y4[i] = y[i] + h * s4;
        }

        // Maximum error across state vector
        let mut max_err: f64 = 0.0;
        for i in 0..n {
            let err = (y5[i] - y4[i]).abs();
            if err > max_err {
                max_err = err;
            }
        }

        if max_err <= tolerance || h.abs() <= 1e-12 {
            t += h;
            y.copy_from_slice(&y5);
            times.push(t);
            traj.push(y.clone());
        }

        // Adjust step size
        let factor = 0.9 * (tolerance / (max_err + 1e-15)).powf(0.2);
        let factor_clamped = factor.clamp(0.2, 5.0);
        h *= factor_clamped;
    }

    (times, traj)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rk4_exponential_decay() {
        // dy/dt = -y, y(0) = 1 => y(1) = 1/e ~ 0.367879441
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| {
            dy[0] = -y[0];
        };
        let (ts, ys) = ode_rk4(f, 0.0, &[1.0], 1.0, 50);
        let final_y = ys.last().unwrap()[0];
        assert!((final_y - (-1.0_f64).exp()).abs() < 1e-5);
        assert_eq!(ts.len(), 51);
    }

    #[test]
    fn test_rk45_adaptive() {
        let f = |_t: f64, y: &[f64], dy: &mut [f64]| {
            dy[0] = -y[0];
        };
        let (_ts, ys) = ode_rk45(f, 0.0, &[1.0], 1.0, 1e-6);
        let final_y = ys.last().unwrap()[0];
        assert!((final_y - (-1.0_f64).exp()).abs() < 1e-6);
    }
}
