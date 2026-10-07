//! Klatter: a text-to-speech engine based on Klatt (1980, 1987).
//!
//! Pipeline: text normalization -> pronunciation (dictionary, then
//! letter-to-sound rules) -> segmental durations -> parameter frames with F0
//! -> cascade/parallel formant synthesis.

// Signal-processing loops index several parallel per-frame arrays at once,
// which reads more clearly than zipped iterators.
#![allow(clippy::needless_range_loop)]

pub mod klatt;
pub mod lexicon;
pub mod lts;
pub mod morph;
pub mod params;
pub mod phoneme;
pub mod prosody;
pub mod text;
pub mod utterance;
pub mod wav;

pub const DEFAULT_SAMPLE_RATE: u32 = 16_000;

/// Peak level after normalization, leaving headroom below full scale.
const PEAK: f64 = 0.89 * i16::MAX as f64;

/// Klatter's natural speaking rate, averaged over two long readings including pauses.
pub const DEFAULT_WPM: f64 = 220.0;

pub fn synthesize_frames(input: &str) -> Vec<klatt::Frame> {
    synthesize_frames_at_rate(input, DEFAULT_WPM)
}

/// `wpm` must be positive and finite.
pub fn synthesize_frames_at_rate(input: &str, wpm: f64) -> Vec<klatt::Frame> {
    let tokens = text::normalize(input);
    let mut segs = utterance::build(&tokens);
    prosody::assign_durations(&mut segs);
    let scale = DEFAULT_WPM / wpm;
    for seg in &mut segs {
        seg.dur_ms *= scale;
    }
    params::build_frames(&segs)
}

pub fn synthesize(input: &str, sample_rate: u32) -> Vec<i16> {
    synthesize_at_rate(input, sample_rate, DEFAULT_WPM)
}

/// `wpm` must be positive and finite.
pub fn synthesize_at_rate(input: &str, sample_rate: u32, wpm: f64) -> Vec<i16> {
    let frames = synthesize_frames_at_rate(input, wpm);
    let raw = klatt::Synth::new(sample_rate).render(&frames);
    let peak = raw.iter().fold(0.0f64, |m, x| m.max(x.abs()));
    let scale = if peak > 0.0 { PEAK / peak } else { 0.0 };
    raw.iter().map(|x| (x * scale).round() as i16).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn produces_audible_speech_of_plausible_length() {
        let audio = synthesize(
            "The quick brown fox jumps over the lazy dog.",
            DEFAULT_SAMPLE_RATE,
        );
        let secs = audio.len() as f64 / DEFAULT_SAMPLE_RATE as f64;
        assert!((1.5..5.0).contains(&secs), "{secs} s");
        let rms =
            (audio.iter().map(|&s| (s as f64).powi(2)).sum::<f64>() / audio.len() as f64).sqrt();
        assert!(rms > 1000.0, "rms {rms}");
    }

    #[test]
    fn normalization_variants_produce_identical_frames() {
        for (input, equivalent) in [
            ("don\u{2019}t", "don't"),
            ("Klatt\u{2019}s", "Klatt's"),
            (
                "1,234 people",
                "one thousand two hundred thirty four people",
            ),
            ("-5", "minus five"),
        ] {
            assert!(
                synthesize_frames(input) == synthesize_frames(equivalent),
                "{input}"
            );
        }
    }

    #[test]
    fn unaccented_questions_have_distinct_pitch_tracks() {
        for word in ["Are you", "Who", "To whom"] {
            let question = synthesize_frames(&format!("{word}?"));
            let statement = synthesize_frames(&format!("{word}."));
            assert_eq!(question.len(), statement.len());
            assert!(
                question
                    .iter()
                    .zip(&statement)
                    .any(|(q, s)| q.av > 0.0 && q.f0 > s.f0 + 10.0)
            );
            for (q, s) in question.iter().zip(statement) {
                assert_eq!(klatt::Frame { f0: s.f0, ..*q }, s);
            }
        }
    }

    #[test]
    fn rate_scales_duration() {
        let text = "Alice was beginning to get very tired of sitting by her sister on the bank.";
        let normal = synthesize_frames(text).len() as f64;
        for factor in [0.5, 2.0, 3.0] {
            let len = synthesize_frames_at_rate(text, DEFAULT_WPM * factor).len() as f64;
            assert!(
                (len * factor / normal - 1.0).abs() < 0.1,
                "{factor}: {len} vs {normal}"
            );
        }
    }

    #[test]
    fn empty_input_is_silent() {
        assert!(synthesize("", DEFAULT_SAMPLE_RATE).iter().all(|&s| s == 0));
    }
}
