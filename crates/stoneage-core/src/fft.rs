//! Port of `FastFourierTransform.fftMag` (radix-2, magnitude spectrum).
//!
//! Arithmetic is done in `f32` exactly like the Java code (twiddle angle in float,
//! `Math.cos/sin/sqrt` evaluated in double and narrowed to float).

/// `FastFourierTransform.bitrev(j)` for `nu` bits.
fn bitrev(j: i32, nu: i32) -> i32 {
    let mut j2 = j;
    let mut k = 0i32;
    for _ in 1..=nu {
        let j1 = j2 / 2;
        k = 2 * k + j2 - 2 * j1;
        j2 = j1;
    }
    k
}

/// Magnitude spectrum of `x` (length must be a power of two). Same output as the Java version:
/// `n/2` values, `mag[0] = |X0| / n`, `mag[i] = 2 * |Xi| / n` for `0 < i < n/2`.
pub fn fft_mag(x: &[f32]) -> Vec<f32> {
    let n = x.len() as i32;
    if n == 0 {
        return Vec::new();
    }
    let nu = ((n as f64).ln() / 2f64.ln()) as i32;
    let mut n2 = n / 2;
    let mut nu1 = nu - 1;
    let nn = n as usize;
    let mut xre: Vec<f32> = x.to_vec();
    let mut xim: Vec<f32> = vec![0.0; nn];
    let mut mag: Vec<f32> = vec![0.0; (n / 2) as usize];

    let mut k: i32 = 0;
    for _l in 1..=nu {
        while k < n {
            for _i in 1..=n2 {
                let p = bitrev(k >> nu1, nu) as f32;
                let arg = 6.283_185_5_f32 * p / n as f32;
                let c = (arg as f64).cos() as f32;
                let s = (arg as f64).sin() as f32;
                let ku = k as usize;
                let kn2 = (k + n2) as usize;
                let tr = xre[kn2] * c + xim[kn2] * s;
                let ti = xim[kn2] * c - xre[kn2] * s;
                xre[kn2] = xre[ku] - tr;
                xim[kn2] = xim[ku] - ti;
                xre[ku] += tr;
                xim[ku] += ti;
                k += 1;
            }
            k += n2;
        }
        k = 0;
        nu1 -= 1;
        n2 /= 2;
    }

    for k in 0..n {
        let r = bitrev(k, nu);
        if r > k {
            let (ku, ru) = (k as usize, r as usize);
            xre.swap(ku, ru);
            xim.swap(ku, ru);
        }
    }

    if !mag.is_empty() {
        mag[0] = ((xre[0] * xre[0] + xim[0] * xim[0]) as f64).sqrt() as f32 / n as f32;
    }
    for i in 1..(n / 2) as usize {
        mag[i] = 2.0 * ((xre[i] * xre[i] + xim[i] * xim[i]) as f64).sqrt() as f32 / n as f32;
    }
    mag
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bitrev_works() {
        assert_eq!(bitrev(1, 3), 4);
        assert_eq!(bitrev(3, 3), 6);
        assert_eq!(bitrev(6, 3), 3);
    }

    #[test]
    fn sine_peak() {
        let n = 1024;
        let bin = 37;
        let amp = 3.0f32;
        let x: Vec<f32> = (0..n)
            .map(|i| amp * (2.0 * std::f32::consts::PI * bin as f32 * i as f32 / n as f32).sin())
            .collect();
        let m = fft_mag(&x);
        assert_eq!(m.len(), n / 2);
        let (imax, vmax) = m
            .iter()
            .enumerate()
            .fold((0, 0.0f32), |a, (i, &v)| if v > a.1 { (i, v) } else { a });
        assert_eq!(imax, bin);
        assert!((vmax - amp).abs() < 1e-2, "peak {vmax}");
        for (i, &v) in m.iter().enumerate() {
            if i != bin {
                assert!(v < 1e-2, "leak at {i}: {v}");
            }
        }
    }

    #[test]
    fn dc_component() {
        let x = vec![2.0f32; 64];
        let m = fft_mag(&x);
        assert!((m[0] - 2.0).abs() < 1e-5);
        assert!(m[1..].iter().all(|v| v.abs() < 1e-4));
    }
}
