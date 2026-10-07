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

/// Output gain that puts the loudest speech in two long readings at 89% of full scale.
const GAIN: f64 = 200_000.0;
/// Keeps extreme pitch and inflection settings above the range where pulses become clicks.
const MIN_F0: f64 = 30.0;

/// Klatter's natural speaking rate, averaged over two long readings including pauses.
pub const DEFAULT_WPM: f64 = 220.0;

/// Speaking settings in physical units, so each host maps its own scales onto them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Voice {
    /// Words per minute; must be positive and finite.
    pub wpm: f64,
    /// The pitch baseline in Hz; the whole contour moves with it.
    pub pitch_hz: f64,
    /// Pitch-range multiplier: 0 is monotone and 1 is natural.
    pub inflection: f64,
    /// Linear amplitude multiplier: 1 is full level.
    pub volume: f64,
}

impl Default for Voice {
    fn default() -> Self {
        Voice {
            wpm: DEFAULT_WPM,
            pitch_hz: prosody::BASE_F0,
            inflection: 1.0,
            volume: 1.0,
        }
    }
}

/// Input to [`speak`]: text, with marks the caller wants located in the audio.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item<'a> {
    Text(&'a str),
    Mark(u32),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Speech {
    pub samples: Vec<i16>,
    /// Each mark with the index of the sample it precedes, in input order.
    pub marks: Vec<(u32, usize)>,
}

pub fn synthesize_frames(input: &str) -> Vec<klatt::Frame> {
    frames_with_marks(&[Item::Text(input)], &Voice::default()).0
}

pub fn synthesize(input: &str, sample_rate: u32) -> Vec<i16> {
    speak(&[Item::Text(input)], sample_rate, &Voice::default()).samples
}

/// Speaks all items as one utterance, so text on either side of a mark keeps
/// its phrasing.
pub fn speak(items: &[Item], sample_rate: u32, voice: &Voice) -> Speech {
    let (frames, frame_marks) = frames_with_marks(items, voice);
    let raw = klatt::Synth::new(sample_rate).render(&frames);
    let gain = GAIN * voice.volume;
    Speech {
        samples: raw
            .iter()
            .map(|x| (x * gain).round().clamp(i16::MIN as f64, i16::MAX as f64) as i16)
            .collect(),
        marks: frame_marks
            .into_iter()
            .map(|(id, frame)| {
                (
                    id,
                    klatt::Synth::frame_start(frame, sample_rate).min(raw.len()),
                )
            })
            .collect(),
    }
}

fn frames_with_marks(items: &[Item], voice: &Voice) -> (Vec<klatt::Frame>, Vec<(u32, usize)>) {
    let mut tokens = Vec::new();
    for item in items {
        match *item {
            Item::Text(t) => tokens.extend(text::normalize_fragment(t)),
            Item::Mark(id) => tokens.push(text::Token::Mark(id)),
        }
    }
    text::end_utterance(&mut tokens);
    let (mut segs, seg_marks) = utterance::build_with_marks(&tokens);
    prosody::assign_durations(&mut segs);
    let scale = DEFAULT_WPM / voice.wpm;
    for seg in &mut segs {
        seg.dur_ms *= scale;
    }
    let (starts, _) = params::frame_layout(&segs);
    let mut frames = params::build_frames(&segs);
    let pitch_scale = voice.pitch_hz / prosody::BASE_F0;
    for f in &mut frames {
        // Written so the natural settings leave F0 bit-for-bit unchanged.
        let shaped = f.f0 * voice.inflection + prosody::BASE_F0 * (1.0 - voice.inflection);
        f.f0 = (shaped * pitch_scale).max(MIN_F0);
    }
    let marks = seg_marks
        .into_iter()
        .map(|(id, seg)| (id, starts.get(seg).copied().unwrap_or(frames.len())))
        .collect();
    (frames, marks)
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

    fn frames(text: &str, voice: Voice) -> Vec<klatt::Frame> {
        frames_with_marks(&[Item::Text(text)], &voice).0
    }

    const SENTENCE: &str =
        "Alice was beginning to get very tired of sitting by her sister on the bank.";

    #[test]
    fn rate_scales_duration() {
        let normal = synthesize_frames(SENTENCE).len() as f64;
        for factor in [0.5, 2.0, 3.0] {
            let len = frames(
                SENTENCE,
                Voice {
                    wpm: DEFAULT_WPM * factor,
                    ..Voice::default()
                },
            )
            .len() as f64;
            assert!(
                (len * factor / normal - 1.0).abs() < 0.1,
                "{factor}: {len} vs {normal}"
            );
        }
    }

    #[test]
    fn pitch_moves_and_inflection_stretches_the_contour() {
        let f0 = |voice| {
            frames(SENTENCE, voice)
                .iter()
                .map(|f| f.f0)
                .collect::<Vec<_>>()
        };
        let natural = f0(Voice::default());
        let higher = f0(Voice {
            pitch_hz: prosody::BASE_F0 * 1.5,
            ..Voice::default()
        });
        assert!(
            natural
                .iter()
                .zip(&higher)
                .all(|(a, b)| (b / a - 1.5).abs() < 1e-9)
        );
        let flat = f0(Voice {
            inflection: 0.0,
            ..Voice::default()
        });
        assert!(flat.iter().all(|&f| f == prosody::BASE_F0));
        let wide = f0(Voice {
            inflection: 2.0,
            ..Voice::default()
        });
        let range = |v: &[f64]| {
            v.iter().cloned().fold(f64::MIN, f64::max) - v.iter().cloned().fold(f64::MAX, f64::min)
        };
        assert!((range(&wide) / range(&natural) - 2.0).abs() < 1e-9);
    }

    #[test]
    fn volume_scales_amplitude() {
        let full = speak(
            &[Item::Text(SENTENCE)],
            DEFAULT_SAMPLE_RATE,
            &Voice::default(),
        )
        .samples;
        let half = speak(
            &[Item::Text(SENTENCE)],
            DEFAULT_SAMPLE_RATE,
            &Voice {
                volume: 0.5,
                ..Voice::default()
            },
        )
        .samples;
        assert!(
            full.iter()
                .zip(&half)
                .all(|(&a, &b)| (a as f64 / 2.0 - b as f64).abs() <= 1.0)
        );
    }

    #[test]
    fn marks_do_not_change_the_audio_and_land_on_word_boundaries() {
        let plain = synthesize("Hello there. How are you?", DEFAULT_SAMPLE_RATE);
        let marked = speak(
            &[
                Item::Mark(1),
                Item::Text("Hello there."),
                Item::Mark(2),
                Item::Text("How are you?"),
                Item::Mark(3),
            ],
            DEFAULT_SAMPLE_RATE,
            &Voice::default(),
        );
        assert_eq!(marked.samples, plain);
        let ids: Vec<u32> = marked.marks.iter().map(|m| m.0).collect();
        assert_eq!(ids, [1, 2, 3]);
        let at = |id: u32| marked.marks.iter().find(|m| m.0 == id).unwrap().1;
        assert!(at(1) < at(2) && at(2) < at(3) && at(3) <= plain.len());
        let first = speak(
            &[Item::Text("Hello there.")],
            DEFAULT_SAMPLE_RATE,
            &Voice::default(),
        )
        .samples
        .len();
        assert!((at(2) as f64 - first as f64).abs() < 0.2 * DEFAULT_SAMPLE_RATE as f64);
    }

    #[test]
    fn text_split_by_a_mark_keeps_its_phrasing() {
        let whole = synthesize_frames("one two three");
        let (split, marks) = frames_with_marks(
            &[Item::Text("one two"), Item::Mark(7), Item::Text("three")],
            &Voice::default(),
        );
        assert_eq!(split, whole);
        assert_eq!(marks.len(), 1);
    }

    #[test]
    fn empty_input_is_silent() {
        assert!(synthesize("", DEFAULT_SAMPLE_RATE).iter().all(|&s| s == 0));
    }
}
