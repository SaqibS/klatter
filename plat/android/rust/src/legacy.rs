//! Android 2.2 to 3.2's engine plugin (jni/legacy_plugin.c): its settings and
//! its speaking.
//!
//! The plugin runs in Android's own speech service, not in the app, so it can't
//! read the app's settings. The app writes them to files/klatter.conf instead
//! (Settings.exportForOldAndroid), readable by the service, and the plugin reads
//! that file again before an utterance whenever it has changed. One
//! "name = value" a line; anything unknown or out of range is left at its default.

use engine::{Item, Pronunciations, Quality, Voice, speak_stream_with_pronunciations};
use std::ffi::{CStr, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::time::SystemTime;

/// Android before 5.0 plays 48 kHz at most.
const SAMPLE_RATES: [u32; 15] =
    [8000, 10000, 11000, 11025, 12000, 16000, 20000, 22000, 22050, 24000, 32000, 40000, 44000, 44100, 48000];
const DEFAULT_SAMPLE_RATE: u32 = 16000;

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub rate: f64,
    pub follow_rate: bool,
    /// An exact base pitch, or None for Klatter's natural one.
    pub pitch: Option<f64>,
    pub follow_pitch: bool,
    pub inflection: f64,
    pub volume: f64,
    pub sample_rate: u32,
    pub quality: Quality,
    pub dictionaries: Vec<String>,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            rate: engine::DEFAULT_WPM,
            follow_rate: true,
            pitch: None,
            follow_pitch: true,
            inflection: 100.0,
            volume: 100.0,
            sample_rate: DEFAULT_SAMPLE_RATE,
            quality: Quality::default(),
            dictionaries: Vec::new(),
        }
    }
}

fn number(value: &str, low: f64, high: f64) -> Option<f64> {
    value.trim().parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.clamp(low, high))
}

fn yes(value: &str) -> Option<bool> {
    match value.trim().to_ascii_lowercase().as_str() {
        "yes" | "on" | "true" | "1" => Some(true),
        "no" | "off" | "false" | "0" => Some(false),
        _ => None,
    }
}

impl Config {
    pub fn parse(text: &str) -> Config {
        let mut c = Config::default();
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else { continue };
            let (key, value) = (key.trim(), value.trim());
            match key {
                "rate" => c.rate = number(value, 30.0, 1920.0).unwrap_or(c.rate),
                "follow-rate" => c.follow_rate = yes(value).unwrap_or(c.follow_rate),
                "pitch" => c.pitch = number(value, 40.0, 400.0),
                "follow-pitch" => c.follow_pitch = yes(value).unwrap_or(c.follow_pitch),
                "inflection" => c.inflection = number(value, 0.0, 200.0).unwrap_or(c.inflection),
                "volume" => c.volume = number(value, 0.0, 200.0).unwrap_or(c.volume),
                "sample-rate" => {
                    if let Some(r) = number(value, 0.0, 1e6).map(|r| r as u32).filter(|r| SAMPLE_RATES.contains(r)) {
                        c.sample_rate = r;
                    }
                }
                "dictionary" => {
                    if !value.is_empty() {
                        c.dictionaries.push(value.to_string());
                    }
                }
                code => {
                    // A voice-quality setting by its code (fl, hs, br, f4, ...).
                    if let Some(v) = number(value, f64::MIN, f64::MAX) {
                        c.quality.set(code, v);
                    }
                }
            }
        }
        c
    }

    /// The voice for an utterance, with Android's rate and pitch (100 normal).
    pub fn voice(&self, system_rate: c_int, system_pitch: c_int) -> Voice {
        let rate_scale = if self.follow_rate && system_rate > 0 { f64::from(system_rate) / 100.0 } else { 1.0 };
        let pitch_scale = if self.follow_pitch && system_pitch > 0 { f64::from(system_pitch) / 100.0 } else { 1.0 };
        Voice {
            wpm: (self.rate * rate_scale).clamp(30.0, 1920.0),
            pitch_hz: (self.pitch.unwrap_or(100.0) * pitch_scale).clamp(40.0, 400.0),
            inflection: self.inflection / 100.0,
            volume: self.volume / 100.0,
            quality: self.quality,
        }
    }
}

/// Loads dictionary text (a word and its ARPAbet sounds a line) into `words`.
pub fn load_dictionary(words: &mut Pronunciations, text: &str) {
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if let Some((word, phones)) = line.split_once(char::is_whitespace) {
            let _ = words.insert(word, phones.trim());
        }
    }
}

pub struct Legacy {
    path: Option<String>,
    stamp: Option<(SystemTime, u64)>,
    config: Config,
    words: Pronunciations,
}

impl Legacy {
    fn refresh(&mut self) {
        let Some(path) = &self.path else { return };
        let stamp = std::fs::metadata(path).ok().and_then(|m| Some((m.modified().ok()?, m.len())));
        if stamp.is_some() && stamp == self.stamp {
            return;
        }
        self.stamp = stamp;
        self.config = std::fs::read_to_string(path).map(|t| Config::parse(&t)).unwrap_or_default();
        let mut words = Pronunciations::default();
        for file in &self.config.dictionaries {
            if let Ok(text) = std::fs::read_to_string(file) {
                load_dictionary(&mut words, &text);
            }
        }
        self.words = words;
    }
}

/// Settings read from `path` (files/klatter.conf); a null path gives the defaults.
///
/// # Safety
/// `path` must be null or a NUL-terminated string.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_legacy_new(path: *const c_char) -> *mut Legacy {
    let path = (!path.is_null()).then(|| unsafe { CStr::from_ptr(path) }.to_string_lossy().into_owned());
    let legacy = Legacy { path, stamp: None, config: Config::default(), words: Pronunciations::default() };
    Box::into_raw(Box::new(legacy))
}

/// # Safety
/// `legacy` must be null or from `klatter_legacy_new`, and not used after.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_legacy_free(legacy: *mut Legacy) {
    if !legacy.is_null() {
        drop(unsafe { Box::from_raw(legacy) });
    }
}

/// Reads the settings again if the file changed; returns the sample rate to speak at.
///
/// # Safety
/// `legacy` must be null or from `klatter_legacy_new`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_legacy_refresh(legacy: *mut Legacy) -> c_int {
    let Some(l) = (unsafe { legacy.as_mut() }) else { return DEFAULT_SAMPLE_RATE as c_int };
    catch_unwind(AssertUnwindSafe(|| {
        l.refresh();
        l.config.sample_rate as c_int
    }))
    .unwrap_or(DEFAULT_SAMPLE_RATE as c_int)
}

/// Receives samples; returns nonzero to stop.
pub type LegacyAudio = unsafe extern "C" fn(user: *mut c_void, samples: *const i16, count: c_int) -> c_int;

/// Speaks UTF-8 text with the settings (call `klatter_legacy_refresh` first) and
/// Android's rate and pitch, at the settings' sample rate. Returns 0 when all was
/// spoken, 1 when `audio` stopped it, -1 on an error.
///
/// # Safety
/// `legacy` must come from `klatter_legacy_new`, `text` must be NUL-terminated,
/// and `audio` must be safe to call with `user`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_legacy_speak(
    legacy: *const Legacy,
    text: *const c_char,
    system_rate: c_int,
    system_pitch: c_int,
    audio: Option<LegacyAudio>,
    user: *mut c_void,
) -> c_int {
    let (Some(l), false, Some(audio)) = (unsafe { legacy.as_ref() }, text.is_null(), audio) else { return -1 };
    let text = unsafe { CStr::from_ptr(text) }.to_string_lossy();
    catch_unwind(AssertUnwindSafe(|| {
        let voice = l.config.voice(system_rate, system_pitch);
        let done = speak_stream_with_pronunciations(
            &[Item::Text(&text)],
            l.config.sample_rate,
            &voice,
            &l.words,
            |samples, _| samples.is_empty() || unsafe { audio(user, samples.as_ptr(), samples.len() as c_int) == 0 },
        );
        if done { 0 } else { 1 }
    }))
    .unwrap_or(-1)
}

#[cfg(test)]
mod tests {
    use super::*;
    use engine::PARAMETERS;

    #[test]
    fn reads_the_settings_the_app_writes() {
        let c = Config::parse(
            "# Written by the Klatter app\nrate = 400\nfollow-rate = no\npitch = 130\nfollow-pitch = yes\n\
             inflection = 150\nvolume = 80\nsample-rate = 22050\nfl = 25\nhs = 110\ndictionary = /x/words.txt\n",
        );
        assert_eq!((c.rate, c.follow_rate, c.pitch, c.inflection, c.volume, c.sample_rate), (400.0, false, Some(130.0), 150.0, 80.0, 22050));
        assert_eq!((c.quality.get("fl"), c.quality.get("hs")), (Some(25.0), Some(110.0)));
        assert_eq!(c.dictionaries, vec!["/x/words.txt".to_string()]);
        let v = c.voice(200, 200);
        assert_eq!((v.wpm, v.pitch_hz), (400.0, 260.0));
    }

    #[test]
    fn defaults_and_limits() {
        let c = Config::parse("rate = 99999\nsample-rate = 96000\npitch = natural\nxx = 3\n");
        assert_eq!(c.rate, 1920.0);
        assert_eq!(c.sample_rate, DEFAULT_SAMPLE_RATE);
        assert_eq!(c.pitch, None);
        assert_eq!(c.quality, Quality::default());
        assert_eq!(c.quality.flutter(), 0.0);
        assert_eq!(PARAMETERS.len(), 17);
    }

    #[test]
    fn speaks_through_the_plugin_interface() {
        unsafe extern "C" fn count(user: *mut c_void, _: *const i16, n: c_int) -> c_int {
            unsafe { *(user as *mut usize) += n as usize };
            0
        }
        let l = unsafe { klatter_legacy_new(std::ptr::null()) };
        let mut n = 0usize;
        let text = c"Hello there.";
        let r = unsafe { klatter_legacy_speak(l, text.as_ptr(), 100, 100, Some(count), &mut n as *mut usize as *mut c_void) };
        unsafe { klatter_legacy_free(l) };
        assert_eq!(r, 0);
        assert!(n > 8000, "{n} samples");
    }
}
