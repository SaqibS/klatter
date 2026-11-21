//! Voice quality: the KLSYN88 source and resonator settings that Klatter
//! otherwise fixes, and the size of its pitch movements, as settings a host can
//! change. Every one defaults to the value the engine always used, so the
//! default voice is unchanged, except flutter, which is off unless turned up.
//!
//! The table [`PARAMETERS`] is the one list every host reads (the C interface,
//! the NVDA add-on and the apps), so a setting added here reaches all of them.

/// One setting, as hosts show it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Parameter {
    /// Short code, as in Klattara's `[:cv]` where the meaning is the same.
    pub code: &'static str,
    pub name: &'static str,
    pub min: f64,
    pub max: f64,
    pub default: f64,
    /// A sensible step for a slider or stepper.
    pub step: f64,
    /// "percent", "hertz", "decibels", "hertz per second", or "" for a plain number.
    pub unit: &'static str,
    pub help: &'static str,
}

/// Every setting, in the order hosts show them. The indexes are the C
/// interface's, so new settings go at the end.
pub const PARAMETERS: &[Parameter] = &[
    Parameter {
        code: "fl",
        name: "Flutter",
        min: 0.0,
        max: 100.0,
        default: 0.0,
        step: 5.0,
        unit: "percent",
        help: "Slow, uneven wavering of the pitch, as in KLSYN88. Klatt and Klatt found 25 natural; off by default.",
    },
    Parameter {
        code: "hs",
        name: "Head size",
        min: 64.0,
        max: 135.0,
        default: 100.0,
        step: 1.0,
        unit: "percent",
        help: "Percent of normal. Larger lowers every formant, as a longer vocal tract does; smaller sounds younger.",
    },
    Parameter {
        code: "rg",
        name: "Roughness",
        min: 0.0,
        max: 100.0,
        default: 0.0,
        step: 5.0,
        unit: "percent",
        help: "Double pulsing (KLSYN88's diplophonia): every other pulse of the voice is weaker and late.",
    },
    Parameter {
        code: "br",
        name: "Breathiness",
        min: 0.0,
        max: 72.0,
        default: 0.0,
        step: 2.0,
        unit: "decibels",
        help: "Breath noise through the voice, in decibels. 0 is none beyond what each sound has.",
    },
    Parameter {
        code: "oq",
        name: "Open quotient",
        min: 20.0,
        max: 90.0,
        default: 50.0,
        step: 1.0,
        unit: "percent",
        help: "Percent of each pitch period the vocal folds are open. Higher is softer and fuller, lower brighter and buzzier.",
    },
    Parameter {
        code: "tl",
        name: "Spectral tilt",
        min: 0.0,
        max: 41.0,
        default: 0.0,
        step: 1.0,
        unit: "decibels",
        help: "Extra softening of the voice's high frequencies, in decibels down at 3 kilohertz.",
    },
    Parameter {
        code: "vvl",
        name: "Voicing level",
        min: 0.0,
        max: 200.0,
        default: 100.0,
        step: 5.0,
        unit: "percent",
        help: "Percent of normal: vowels and everything the vocal folds sound. 0 with some breathiness whispers.",
    },
    Parameter {
        code: "fvl",
        name: "Frication level",
        min: 0.0,
        max: 200.0,
        default: 100.0,
        step: 5.0,
        unit: "percent",
        help: "Percent of normal: the hiss of S and F, and the bursts of stops.",
    },
    Parameter {
        code: "avl",
        name: "Aspiration level",
        min: 0.0,
        max: 200.0,
        default: 100.0,
        step: 5.0,
        unit: "percent",
        help: "Percent of normal: H, and the puff after P, T and K.",
    },
    Parameter {
        code: "st",
        name: "Accent size",
        min: 0.0,
        max: 300.0,
        default: 100.0,
        step: 10.0,
        unit: "percent",
        help: "Percent of normal: how far the pitch rises on each accented word.",
    },
    Parameter {
        code: "fa",
        name: "Baseline fall",
        min: 0.0,
        max: 40.0,
        default: 8.0,
        step: 1.0,
        unit: "hertz per second",
        help: "How fast the pitch's baseline falls through a sentence, in hertz per second.",
    },
    Parameter {
        code: "f4",
        name: "F4",
        min: 2000.0,
        max: 6000.0,
        default: 3250.0,
        step: 50.0,
        unit: "hertz",
        help: "The fourth formant, in hertz. Head size still moves it.",
    },
    Parameter {
        code: "b4",
        name: "F4 bandwidth",
        min: 50.0,
        max: 2000.0,
        default: 200.0,
        step: 10.0,
        unit: "hertz",
        help: "In hertz. Wider is softer.",
    },
    Parameter {
        code: "f5",
        name: "F5",
        min: 2500.0,
        max: 6500.0,
        default: 3700.0,
        step: 50.0,
        unit: "hertz",
        help: "The fifth formant, in hertz.",
    },
    Parameter {
        code: "b5",
        name: "F5 bandwidth",
        min: 50.0,
        max: 2000.0,
        default: 200.0,
        step: 10.0,
        unit: "hertz",
        help: "In hertz.",
    },
    Parameter {
        code: "f6",
        name: "F6",
        min: 3000.0,
        max: 8000.0,
        default: 4990.0,
        step: 50.0,
        unit: "hertz",
        help: "The sixth formant, in hertz. It also shapes how sharp S sounds.",
    },
    Parameter {
        code: "b6",
        name: "F6 bandwidth",
        min: 50.0,
        max: 3000.0,
        default: 500.0,
        step: 10.0,
        unit: "hertz",
        help: "In hertz.",
    },
];

/// Values for every setting in [`PARAMETERS`], by index.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quality {
    values: [f64; PARAMETERS.len()],
}

impl Default for Quality {
    fn default() -> Self {
        let mut values = [0.0; PARAMETERS.len()];
        for (v, p) in values.iter_mut().zip(PARAMETERS) {
            *v = p.default;
        }
        Quality { values }
    }
}

/// The index of a setting's code, if there is one.
pub fn index_of(code: &str) -> Option<usize> {
    PARAMETERS.iter().position(|p| p.code.eq_ignore_ascii_case(code))
}

impl Quality {
    /// A setting's value by code; `None` for an unknown code.
    pub fn get(&self, code: &str) -> Option<f64> {
        index_of(code).map(|i| self.values[i])
    }

    /// Sets a setting by code, kept within its range. Returns false for an
    /// unknown code or a value that is not a number.
    pub fn set(&mut self, code: &str, value: f64) -> bool {
        match index_of(code) {
            Some(i) => self.set_index(i, value),
            None => false,
        }
    }

    pub fn value(&self, index: usize) -> f64 {
        self.values[index]
    }

    /// Sets a setting by index, kept within its range; NaN leaves it as it was.
    pub fn set_index(&mut self, index: usize, value: f64) -> bool {
        let Some(p) = PARAMETERS.get(index) else { return false };
        if value.is_nan() {
            return false;
        }
        self.values[index] = value.clamp(p.min, p.max);
        true
    }

    fn at(&self, code: &str) -> f64 {
        self.get(code).expect("a known code")
    }

    pub fn flutter(&self) -> f64 {
        self.at("fl")
    }
    /// Formant frequencies are multiplied by this: 1 at normal head size.
    pub fn formant_scale(&self) -> f64 {
        100.0 / self.at("hs")
    }
    pub fn accent_scale(&self) -> f64 {
        self.at("st") / 100.0
    }
    pub fn declination(&self) -> f64 {
        self.at("fa")
    }
    /// F4 to F6 and their bandwidths, before head size.
    pub fn upper_formants(&self) -> ([f64; 3], [f64; 3]) {
        (
            [self.at("f4"), self.at("f5"), self.at("f6")],
            [self.at("b4"), self.at("b5"), self.at("b6")],
        )
    }
    /// True when the settings that change frames are all at their defaults,
    /// so the frames can be left exactly as the engine made them.
    pub fn frames_unchanged(&self) -> bool {
        let d = Quality::default();
        ["fl", "hs", "rg", "br", "oq", "tl", "vvl", "fvl", "avl"]
            .iter()
            .all(|c| self.at(c) == d.at(c))
    }
}

/// Decibels for a level in percent of normal; very low for 0.
fn level_db(percent: f64) -> f64 {
    if percent <= 0.0 { -200.0 } else { 20.0 * (percent / 100.0).log10() }
}

/// Adds two noise levels given in dB, as powers.
fn db_sum(a: f64, b: f64) -> f64 {
    if a <= 0.0 {
        return b;
    }
    if b <= 0.0 {
        return a;
    }
    10.0 * (10f64.powf(a / 10.0) + 10f64.powf(b / 10.0)).log10()
}

/// Applies the settings to frames the engine planned. Pitch is the voice's
/// pitch contour before inflection and the pitch setting, as KLSYN88 adds
/// flutter to it; `frame_ms` is the frame period.
pub fn apply(frames: &mut [crate::klatt::Frame], quality: &Quality, frame_ms: f64) {
    if quality.frames_unchanged() {
        return;
    }
    use std::f64::consts::TAU;
    let fl = quality.flutter();
    let scale = quality.formant_scale();
    let roughness = quality.at("rg");
    let breath = quality.at("br");
    let oq_shift = quality.at("oq") - 50.0;
    let tilt = quality.at("tl");
    let voicing = level_db(quality.at("vvl"));
    let frication = level_db(quality.at("fvl"));
    let aspiration = level_db(quality.at("avl"));
    for (i, fr) in frames.iter_mut().enumerate() {
        if fl > 0.0 {
            // KLSYN88's flutter: three slow sinusoids, scaled by FL and F0.
            let t = i as f64 * frame_ms / 1000.0;
            let wobble = (TAU * 12.7 * t).sin() + (TAU * 7.1 * t).sin() + (TAU * 4.7 * t).sin();
            fr.f0 += (fl / 50.0) * (fr.f0 / 100.0) * wobble;
        }
        if scale != 1.0 {
            for k in 0..3 {
                fr.f[k] *= scale;
                fr.b[k] *= scale;
            }
        }
        fr.di = fr.di.max(roughness);
        fr.oq = (fr.oq + oq_shift).clamp(10.0, 99.0);
        fr.tl = (fr.tl + tilt).min(41.0);
        if fr.av > 0.0 {
            fr.av = (fr.av + voicing).max(0.0);
            if breath > 0.0 {
                fr.ah = db_sum(fr.ah, breath);
            }
        }
        if fr.avs > 0.0 {
            fr.avs = (fr.avs + voicing).max(0.0);
        }
        if fr.ah > 0.0 {
            fr.ah = (fr.ah + aspiration).max(0.0);
        }
        if fr.af > 0.0 {
            fr.af = (fr.af + frication).max(0.0);
        }
        if fr.ab > 0.0 {
            fr.ab = (fr.ab + frication).max(0.0);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_are_unique_and_defaults_are_in_range() {
        for (i, p) in PARAMETERS.iter().enumerate() {
            assert!(p.min <= p.default && p.default <= p.max, "{}", p.code);
            assert_eq!(index_of(p.code), Some(i), "{}", p.code);
        }
    }

    #[test]
    fn flutter_is_off_by_default() {
        assert_eq!(Quality::default().flutter(), 0.0);
        assert!(Quality::default().frames_unchanged());
    }

    #[test]
    fn values_stay_in_range() {
        let mut q = Quality::default();
        assert!(q.set("hs", 500.0));
        assert_eq!(q.get("hs"), Some(135.0));
        assert!(q.set("FL", -3.0));
        assert_eq!(q.get("fl"), Some(0.0));
        assert!(!q.set("xx", 1.0));
        assert!(!q.set("fl", f64::NAN));
    }
}
