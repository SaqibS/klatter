//! Cascade/parallel formant synthesizer following Klatt (1980).
//!
//! Voicing (impulse train shaped by RGP/RGZ), the quasi-sinusoidal voice bar
//! (RGS) and aspiration excite the cascade vocal-tract model. Frication excites
//! the parallel branch, whose resonators are normalized so that A2..A6 give the
//! spectral peak level in dB relative to the flat bypass path AB.

use std::f64::consts::PI;

pub const FRAME_MS: f64 = 5.0;

/// Fixed parameters from Klatt (1980), Table I.
const F4: f64 = 3300.0;
const B4: f64 = 250.0;
const F5: f64 = 3750.0;
const B5: f64 = 200.0;
const F6: f64 = 4900.0;
const B6: f64 = 1000.0;
const FNP: f64 = 270.0;
const BNP: f64 = 100.0;
const BNZ: f64 = 100.0;
/// Parallel-branch bandwidths for F2..F6, wide enough to suit frication noise.
const PARALLEL_BW: [f64; 5] = [200.0, 250.0, 320.0, 360.0, B6];

/// Gains that balance the cascade (voiced) and parallel (frication) branches
/// so that equal dB values give perceptually comparable levels.
const CASCADE_GAIN: f64 = 1.0;
const PARALLEL_GAIN: f64 = 0.035;
const ASPIRATION_GAIN: f64 = 0.03;
const VOICE_BAR_GAIN: f64 = 1.0;

/// Parameters for one 5 ms frame. Amplitudes are in dB; 0 means silent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Frame {
    pub f0: f64,
    pub av: f64,
    pub avs: f64,
    pub ah: f64,
    pub af: f64,
    pub ab: f64,
    /// F1..F3 in Hz.
    pub f: [f64; 3],
    /// B1..B3 in Hz.
    pub b: [f64; 3],
    pub fnz: f64,
    /// Parallel amplitudes A2..A6.
    pub a: [f64; 5],
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            f0: 100.0,
            av: 0.0,
            avs: 0.0,
            ah: 0.0,
            af: 0.0,
            ab: 0.0,
            f: [500.0, 1500.0, 2500.0],
            b: [60.0, 90.0, 150.0],
            fnz: FNP,
            a: [0.0; 5],
        }
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct Resonator {
    a: f64,
    b: f64,
    c: f64,
    y1: f64,
    y2: f64,
}

impl Resonator {
    fn set(&mut self, f: f64, bw: f64, sr: f64) {
        if f >= 0.49 * sr {
            // Above Nyquist the resonator would alias, so pass the signal through.
            self.a = 1.0;
            self.b = 0.0;
            self.c = 0.0;
            return;
        }
        let t = 1.0 / sr;
        let r = (-PI * bw * t).exp();
        self.c = -(r * r);
        self.b = 2.0 * r * (2.0 * PI * f * t).cos();
        self.a = 1.0 - self.b - self.c;
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.a * x + self.b * self.y1 + self.c * self.y2;
        self.y2 = self.y1;
        self.y1 = y;
        y
    }

    fn gain_at(&self, f: f64, sr: f64) -> f64 {
        let w = 2.0 * PI * f / sr;
        let re = 1.0 - self.b * w.cos() - self.c * (2.0 * w).cos();
        let im = self.b * w.sin() + self.c * (2.0 * w).sin();
        self.a.abs() / (re * re + im * im).sqrt()
    }
}

#[derive(Clone, Copy, Default, Debug)]
struct AntiResonator {
    a: f64,
    b: f64,
    c: f64,
    x1: f64,
    x2: f64,
}

impl AntiResonator {
    fn set(&mut self, f: f64, bw: f64, sr: f64) {
        let mut r = Resonator::default();
        r.set(f, bw, sr);
        self.a = 1.0 / r.a;
        self.b = -r.b / r.a;
        self.c = -r.c / r.a;
    }

    fn tick(&mut self, x: f64) -> f64 {
        let y = self.a * x + self.b * self.x1 + self.c * self.x2;
        self.x2 = self.x1;
        self.x1 = x;
        y
    }
}

fn db_to_lin(db: f64) -> f64 {
    if db <= 0.0 {
        0.0
    } else {
        10f64.powf((db - 60.0) / 20.0)
    }
}

pub struct Synth {
    sr: f64,
    rgp: Resonator,
    rgp_bar: Resonator,
    rgz: AntiResonator,
    rgs: Resonator,
    rnp: Resonator,
    rnz: AntiResonator,
    cascade: [Resonator; 5],
    parallel: [Resonator; 5],
    parallel_norm: [f64; 5],
    rng: u32,
    noise_lp: f64,
    period_left: usize,
    period_len: usize,
    rad_prev: f64,
}

impl Synth {
    pub fn new(sample_rate: u32) -> Self {
        let sr = sample_rate as f64;
        let mut s = Synth {
            sr,
            rgp: Resonator::default(),
            rgp_bar: Resonator::default(),
            rgz: AntiResonator::default(),
            rgs: Resonator::default(),
            rnp: Resonator::default(),
            rnz: AntiResonator::default(),
            cascade: [Resonator::default(); 5],
            parallel: [Resonator::default(); 5],
            parallel_norm: [0.0; 5],
            rng: 0x2545_F491,
            noise_lp: 0.0,
            period_left: 0,
            period_len: 1,
            rad_prev: 0.0,
        };
        s.rgp.set(0.0, 100.0, sr);
        s.rgp_bar.set(0.0, 100.0, sr);
        s.rgz.set(1500.0, 6000.0, sr);
        s.rgs.set(0.0, 200.0, sr);
        s.rnp.set(FNP, BNP, sr);
        s
    }

    fn uniform(&mut self) -> f64 {
        // xorshift32 keeps output deterministic without external crates.
        let mut x = self.rng;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.rng = x;
        (x as f64 / u32::MAX as f64) - 0.5
    }

    fn gaussian(&mut self) -> f64 {
        (0..16).map(|_| self.uniform()).sum::<f64>() * 0.5
    }

    fn configure(&mut self, fr: &Frame) {
        let sr = self.sr;
        let freqs = [fr.f[0], fr.f[1], fr.f[2], F4, F5];
        let bws = [fr.b[0], fr.b[1], fr.b[2], B4, B5];
        for i in 0..5 {
            self.cascade[i].set(freqs[i], bws[i], sr);
        }
        self.rnz.set(fr.fnz, BNZ, sr);
        let par_freqs = [fr.f[1], fr.f[2], F4, F5, F6];
        for i in 0..5 {
            self.parallel[i].set(par_freqs[i], PARALLEL_BW[i], sr);
            let g = self.parallel[i].gain_at(par_freqs[i], sr);
            self.parallel_norm[i] = if g > 0.0 { 1.0 / g } else { 0.0 };
        }
    }

    /// Renders frames to floating-point samples (unnormalized).
    pub fn render(&mut self, frames: &[Frame]) -> Vec<f64> {
        let spf = self.sr * FRAME_MS / 1000.0;
        let mut out = Vec::with_capacity((frames.len() as f64 * spf) as usize + 1);
        for (i, fr) in frames.iter().enumerate() {
            self.configure(fr);
            let n = ((i + 1) as f64 * spf).floor() as usize - (i as f64 * spf).floor() as usize;
            let av = db_to_lin(fr.av);
            let avs = db_to_lin(fr.avs) * VOICE_BAR_GAIN;
            let ah = db_to_lin(fr.ah) * ASPIRATION_GAIN;
            let af = db_to_lin(fr.af);
            let ab = db_to_lin(fr.ab);
            let pa: [f64; 5] = std::array::from_fn(|k| db_to_lin(fr.a[k]) * self.parallel_norm[k]);
            let voiced = av > 0.0 || avs > 0.0;
            for _ in 0..n {
                let mut impulse = 0.0;
                if self.period_left == 0 {
                    let f0 = if fr.f0 > 20.0 { fr.f0 } else { 100.0 };
                    self.period_len = ((self.sr / f0).round() as usize).max(1);
                    self.period_left = self.period_len;
                    impulse = 1.0;
                }
                self.period_left -= 1;
                // Amplitudes scale the impulse rather than the filtered
                // waveform so voicing onsets and offsets fall on pulse
                // boundaries instead of making a step mid-pulse.
                let pulse = impulse * self.period_len as f64;
                let voice = self.rgz.tick(self.rgp.tick(pulse * av));
                let voice_bar = self.rgs.tick(self.rgp_bar.tick(pulse * avs));

                let r = self.gaussian();
                self.noise_lp = r + 0.75 * self.noise_lp;
                let mut noise = self.noise_lp;
                // Klatt modulates noise by 50% during the closed half of each
                // glottal period to give voiced fricatives their buzzy quality.
                if voiced && self.period_left < self.period_len / 2 {
                    noise *= 0.5;
                }

                let mut x = voice + voice_bar + noise * ah;
                x = self.rnp.tick(x);
                x = self.rnz.tick(x);
                for k in (0..5).rev() {
                    x = self.cascade[k].tick(x);
                }
                let cascade = x * CASCADE_GAIN;

                let fric = noise * af;
                let mut parallel = fric * ab;
                for k in 0..5 {
                    let sign = if k % 2 == 0 { -1.0 } else { 1.0 };
                    parallel += sign * self.parallel[k].tick(fric * pa[k]);
                }
                let mixed = cascade + parallel * PARALLEL_GAIN;

                let radiated = mixed - self.rad_prev;
                self.rad_prev = mixed;
                out.push(radiated);
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resonator_has_unity_dc_gain() {
        let mut r = Resonator::default();
        r.set(500.0, 60.0, 16000.0);
        assert!((r.gain_at(0.0, 16000.0) - 1.0).abs() < 1e-9);
    }

    #[test]
    fn antiresonator_cancels_matching_resonator() {
        let mut r = Resonator::default();
        let mut z = AntiResonator::default();
        r.set(270.0, 100.0, 16000.0);
        z.set(270.0, 100.0, 16000.0);
        let input: Vec<f64> = (0..200).map(|i| ((i * 7919) % 13) as f64 - 6.0).collect();
        for &x in &input {
            let y = z.tick(r.tick(x));
            assert!((y - x).abs() < 1e-6);
        }
    }

    fn dft_mag(signal: &[f64], f: f64, sr: f64) -> f64 {
        let (mut re, mut im) = (0.0, 0.0);
        for (n, &x) in signal.iter().enumerate() {
            let w = 2.0 * PI * f * n as f64 / sr;
            re += x * w.cos();
            im -= x * w.sin();
        }
        (re * re + im * im).sqrt()
    }

    #[test]
    fn vowel_spectrum_peaks_near_formants() {
        let sr = 16000.0;
        let frame = Frame {
            f0: 100.0,
            av: 60.0,
            f: [700.0, 1200.0, 2600.0],
            b: [80.0, 90.0, 150.0],
            ..Frame::default()
        };
        let mut synth = Synth::new(sr as u32);
        let out = synth.render(&vec![frame; 100]);
        let tail = &out[out.len() / 2..];
        // Harmonics of 100 Hz: the harmonic at F1 must beat ones well away from it.
        let at_f1 = dft_mag(tail, 700.0, sr);
        let away = dft_mag(tail, 1800.0, sr);
        assert!(at_f1 > away * 3.0, "F1 {at_f1} vs trough {away}");
        let at_f2 = dft_mag(tail, 1200.0, sr);
        assert!(at_f2 > away, "F2 {at_f2} vs trough {away}");
    }

    #[test]
    fn silent_frames_render_silence() {
        let mut synth = Synth::new(16000);
        let out = synth.render(&vec![Frame::default(); 10]);
        assert_eq!(out.len(), 800);
        assert!(out.iter().all(|&x| x.abs() < 1e-12));
    }
}
