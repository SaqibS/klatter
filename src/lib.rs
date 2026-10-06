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

pub fn synthesize_frames(input: &str) -> Vec<klatt::Frame> {
    synthesize_frames_with_timing(input, params::Timing::Original)
}

pub fn synthesize_frames_with_timing(input: &str, timing: params::Timing) -> Vec<klatt::Frame> {
    let tokens = text::normalize(input);
    let mut segs = utterance::build(&tokens);
    prosody::assign_durations(&mut segs);
    params::build_frames_with_timing(&segs, timing)
}

pub fn synthesize(input: &str, sample_rate: u32) -> Vec<i16> {
    synthesize_with_timing(input, sample_rate, params::Timing::Original)
}

/// Synthesizes speech with the selected vowel timing policy.
pub fn synthesize_with_timing(input: &str, sample_rate: u32, timing: params::Timing) -> Vec<i16> {
    let frames = synthesize_frames_with_timing(input, timing);
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
    fn empty_input_is_silent() {
        assert!(synthesize("", DEFAULT_SAMPLE_RATE).iter().all(|&s| s == 0));
    }
}
