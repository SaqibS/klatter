//! Synthesis by rule: converts timed phoneme segments into Klatt parameter
//! frames, with formant transitions, stop closures, bursts and aspiration.

use crate::klatt::{FRAME_MS, Frame};
use crate::phoneme::{Class, Ph, Place};
use crate::prosody;
use crate::utterance::Seg;

const ASPIRATION_DB: f64 = 55.0;
const VOICE_BAR_DB: f64 = 47.0;
const NASAL_ZERO_HZ: f64 = 450.0;

/// How strongly a segment imposes its formant targets on its neighbours.
fn dominance(p: Option<Ph>) -> u8 {
    match p.map(Ph::class) {
        Some(Class::Stop | Class::Affricate | Class::Fricative | Class::Nasal) => 3,
        Some(Class::Liquid | Class::Glide) => 2,
        Some(Class::Vowel) => 1,
        _ => 0,
    }
}

/// (frames inside the dominant segment, frames inside the weaker segment).
fn transition_frames(strong: Ph) -> (usize, usize) {
    let ms = |x: f64| (x / FRAME_MS).round() as usize;
    match strong.class() {
        Class::Stop | Class::Affricate => (
            0,
            ms(if strong.place() == Place::Labial {
                60.0
            } else {
                50.0
            }),
        ),
        Class::Fricative | Class::Nasal => (0, ms(40.0)),
        Class::Liquid => (ms(20.0), ms(60.0)),
        Class::Glide => (ms(30.0), ms(70.0)),
        _ => (ms(30.0), ms(30.0)),
    }
}

#[derive(Clone, Copy)]
struct Formants {
    f: [f64; 3],
    b: [f64; 3],
}

fn lerp(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Per-frame formant targets of a segment, before coarticulation.
fn targets(ph: Ph, len: usize) -> Vec<Formants> {
    let spec = ph.spec();
    (0..len)
        .map(|k| match spec.f_end {
            Some(end) => {
                // Diphthongs hold the onset, glide, then hold the offglide.
                let t = ((k as f64 / len.max(1) as f64 - 0.25) / 0.6).clamp(0.0, 1.0);
                Formants {
                    f: std::array::from_fn(|j| lerp(spec.f[j], end[j], t)),
                    b: spec.b,
                }
            }
            None => Formants {
                f: spec.f,
                b: spec.b,
            },
        })
        .collect()
}

pub fn frame_layout(segs: &[Seg]) -> (Vec<usize>, Vec<usize>) {
    let mut starts = Vec::with_capacity(segs.len());
    let mut lens = Vec::with_capacity(segs.len());
    let mut elapsed_ms = 0.0;
    let mut frame = 0usize;
    for s in segs {
        elapsed_ms += s.dur_ms;
        // Cumulative rounding keeps total timing exact despite 5 ms frames.
        let end = ((elapsed_ms / FRAME_MS).round() as usize).max(frame + 1);
        starts.push(frame);
        lens.push(end - frame);
        frame = end;
    }
    (starts, lens)
}

fn formant_tracks(segs: &[Seg], starts: &[usize], lens: &[usize]) -> Vec<Formants> {
    let n = segs.len();
    // Pauses and /h/ borrow formants from a neighbour so nothing jumps oddly.
    let resolved: Vec<Option<Ph>> = (0..n)
        .map(|i| match segs[i].ph {
            Some(Ph::HH) | None => (i + 1..n)
                .chain((0..i).rev())
                .find_map(|j| segs[j].ph.filter(|p| *p != Ph::HH)),
            p => p,
        })
        .collect();
    let mut track: Vec<Formants> = Vec::new();
    for i in 0..n {
        let ph = resolved[i].unwrap_or(Ph::AX);
        let mut t = targets(ph, lens[i]);
        if segs[i].ph == Some(Ph::HH) {
            for f in &mut t {
                f.b = [300.0, 150.0, 200.0];
            }
        }
        track.extend(t);
    }
    let target = track.clone();

    for i in 0..n.saturating_sub(1) {
        let (l, r) = (segs[i].ph, segs[i + 1].ph);
        let (Some(lp), Some(rp)) = (l, r) else {
            continue;
        };
        let (dl, dr) = (dominance(Some(lp)), dominance(Some(rp)));
        let (left_frames, right_frames) = if dl > dr {
            transition_frames(lp)
        } else if dr > dl {
            let (inside, outside) = transition_frames(rp);
            (outside, inside)
        } else if dl == 1 {
            transition_frames(Ph::AX)
        } else {
            (2, 2)
        };
        // Leave at least one frame of each segment untouched by this window.
        let left_frames = left_frames.min(lens[i].saturating_sub(1) / 2);
        let right_frames = right_frames.min(lens[i + 1].saturating_sub(1) / 2);
        let boundary = starts[i + 1];
        let a = boundary - left_frames;
        let b = boundary + right_frames;
        if a == 0 || b >= target.len() || a == b {
            continue;
        }
        let from = target[a - 1];
        let to = target[b];
        let span = (b - a + 1) as f64;
        for k in a..b {
            let t = (k - a + 1) as f64 / span;
            track[k] = Formants {
                f: std::array::from_fn(|j| lerp(from.f[j], to.f[j], t)),
                b: std::array::from_fn(|j| lerp(from.b[j], to.b[j], t)),
            };
        }
    }
    track
}

fn release_frames(ph: Ph) -> usize {
    if ph.place() == Place::Velar { 3 } else { 2 }
}

fn aspiration_frames(segs: &[Seg], i: usize, next_len: usize) -> usize {
    let s = &segs[i];
    let Some(ph) = s.ph else { return 0 };
    let after_s = i > 0 && segs[i - 1].ph == Some(Ph::S) && !s.word_initial;
    let next_sonorant = segs
        .get(i + 1)
        .and_then(|n| n.ph)
        .is_some_and(Ph::is_sonorant);
    if !ph.is_plosive() || ph.is_voiced() || after_s || !next_sonorant {
        return 0;
    }
    let ms = if segs[i + 1].stress > 0 && !segs[i + 1].function_word {
        50.0
    } else {
        25.0
    };
    let ms = if ph.place() == Place::Velar {
        ms + 10.0
    } else {
        ms
    };
    ((ms / FRAME_MS).round() as usize).min(next_len / 2)
}

pub fn build_frames(segs: &[Seg]) -> Vec<Frame> {
    let (starts, lens) = frame_layout(segs);
    let formants = formant_tracks(segs, &starts, &lens);
    let f0 = prosody::f0_contour(segs, &starts, &lens, FRAME_MS);
    let mut frames: Vec<Frame> = (0..formants.len())
        .map(|k| Frame {
            f0: f0[k],
            f: formants[k].f,
            b: formants[k].b,
            ..Frame::default()
        })
        .collect();

    for i in 0..segs.len() {
        let Some(ph) = segs[i].ph else { continue };
        let spec = ph.spec();
        let range = starts[i]..starts[i] + lens[i];
        let len = lens[i];
        match ph.class() {
            Class::Vowel | Class::Liquid | Class::Glide | Class::Nasal => {
                for fr in &mut frames[range] {
                    fr.av = spec.av;
                    if ph.class() == Class::Nasal {
                        fr.fnz = NASAL_ZERO_HZ;
                    }
                }
            }
            Class::Fricative => {
                for fr in &mut frames[range] {
                    fr.av = spec.av;
                    fr.af = spec.af;
                    fr.ab = spec.ab;
                    fr.a = spec.a;
                }
            }
            Class::Aspirate => {
                for fr in &mut frames[range] {
                    fr.ah = 60.0;
                }
            }
            Class::Stop | Class::Affricate => {
                let release = if ph.class() == Class::Affricate {
                    len / 2
                } else {
                    release_frames(ph).min(len.saturating_sub(1)).max(1)
                };
                let closure = len - release;
                for (k, fr) in frames[range].iter_mut().enumerate() {
                    if k < closure {
                        fr.avs = if ph.is_voiced() { VOICE_BAR_DB } else { 0.0 };
                    } else {
                        fr.av = spec.av;
                        fr.af = spec.af;
                        fr.ab = spec.ab;
                        fr.a = spec.a;
                        if ph.class() == Class::Stop && ph.is_voiced() {
                            fr.avs = VOICE_BAR_DB;
                        }
                    }
                }
            }
        }
    }

    // Aspiration replaces voicing at the start of the sonorant after a
    // voiceless plosive, which is how voice-onset time is realized.
    for i in 0..segs.len().saturating_sub(1) {
        let n = aspiration_frames(segs, i, lens[i + 1]);
        for fr in &mut frames[starts[i + 1]..starts[i + 1] + n] {
            fr.av = 0.0;
            fr.ah = ASPIRATION_DB;
            fr.b[0] += 150.0;
        }
    }
    frames
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prosody::assign_durations;
    use crate::text::normalize;
    use crate::utterance::build;

    fn frames_for(text: &str) -> (Vec<Seg>, Vec<usize>, Vec<usize>, Vec<Frame>) {
        let mut segs = build(&normalize(text));
        assign_durations(&mut segs);
        let (starts, lens) = frame_layout(&segs);
        let frames = build_frames(&segs);
        (segs, starts, lens, frames)
    }

    #[test]
    fn layout_covers_all_frames() {
        let (_, starts, lens, frames) = frames_for("Hello there, how are you?");
        assert_eq!(starts.last().unwrap() + lens.last().unwrap(), frames.len());
    }

    #[test]
    fn voiceless_stop_aspirates_following_vowel() {
        let (segs, starts, _, frames) = frames_for("tea");
        let vowel = segs.iter().position(|s| s.ph == Some(Ph::IY)).unwrap();
        let first = frames[starts[vowel]];
        assert_eq!(first.av, 0.0);
        assert!(first.ah > 0.0);
        let stop = segs.iter().position(|s| s.ph == Some(Ph::T)).unwrap();
        assert_eq!(frames[starts[stop]].av, 0.0);
        assert_eq!(frames[starts[stop]].af, 0.0);
    }

    #[test]
    fn formants_move_smoothly_into_vowel() {
        let (segs, starts, lens, frames) = frames_for("bee");
        let vowel = segs.iter().position(|s| s.ph == Some(Ph::IY)).unwrap();
        let f2: Vec<f64> = (starts[vowel]..starts[vowel] + lens[vowel])
            .map(|k| frames[k].f[1])
            .collect();
        assert!(f2[0] < f2[f2.len() - 1]);
        for w in f2.windows(2) {
            assert!((w[1] - w[0]).abs() < 300.0, "{f2:?}");
        }
    }
}
