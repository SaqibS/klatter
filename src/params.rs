//! Synthesis by rule: converts timed phoneme segments into Klatt parameter
//! frames, with formant transitions, stop closures, bursts and aspiration.

use crate::klatt::{FRAME_MS, Frame};
use crate::phoneme::{Class, Ph, Place};
use crate::prosody;
use crate::utterance::Seg;

const ASPIRATION_DB: f64 = 55.0;
const VOICE_BAR_DB: f64 = 47.0;
const NASAL_ZERO_HZ: f64 = 450.0;

/// KLGLOTT88 source settings for a male voice: modal phonation, then the fully
/// breathy values (Klatt & Klatt 1990, Sec. III B and Table XII).
const MODAL_OQ: f64 = 50.0;
const BREATHY_OQ: f64 = 80.0;
const BREATHY_TL: f64 = 20.0;
const BREATHY_AH: f64 = 52.0;
/// Table XIV stimulus 11: B1 200 to 400 Hz and B2 110 to 170 Hz.
const BREATHY_B1_HZ: f64 = 200.0;
const BREATHY_B2_HZ: f64 = 60.0;
/// Fig. 15: AV falls about 10 dB over a breathy offset into silence.
const OFFSET_AV_DROP_DB: f64 = 10.0;
/// Ramp lengths read from Figs. 15 and 16.
const OFFSET_RAMP_MS: f64 = 100.0;
const VOICELESS_RAMP_MS: f64 = 80.0;
const VOICELESS_PEAK: f64 = 0.8;

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

fn ms(x: f64) -> usize {
    (x / FRAME_MS).round() as usize
}

/// Transition timings from Hertz & Huffman (1992) and Hertz, Younes &
/// Zinovieva (1999).
const RELEASE_AFTER_FRICATIVE_MS: f64 = 30.0;
const RELEASE_AFTER_ASPIRATION_MS: f64 = 70.0;
const CLOSURE_AFTER_OFFGLIDE_MS: f64 = 15.0;
const DIPHTHONG_GLIDE_MS: f64 = 80.0;
const DIPHTHONG_OFFGLIDE_MS: f64 = 10.0;
const MIN_ONSET_MS: f64 = 30.0;

fn is_obstruent(p: Ph) -> bool {
    matches!(p.class(), Class::Stop | Class::Affricate | Class::Fricative)
}

/// (frames before, frames after) the boundary between segments `i` and `i + 1`.
fn transition_window(segs: &[Seg], i: usize) -> Option<(usize, usize)> {
    let (lp, rp) = (segs[i].ph?, segs[i + 1].ph?);
    let (dl, dr) = (dominance(Some(lp)), dominance(Some(rp)));
    Some(if dl > dr {
        // Formants move during aspiration, so the release transition spans it.
        let release = match lp.class() {
            Class::Fricative => Some(RELEASE_AFTER_FRICATIVE_MS),
            Class::Stop if prosody::aspiration_ms(segs, i) > 0.0 => {
                Some(RELEASE_AFTER_ASPIRATION_MS)
            }
            _ => None,
        };
        match release {
            Some(r) => (0, ms(r)),
            None => transition_frames(lp),
        }
    } else if dr > dl {
        let (inside, outside) = transition_frames(rp);
        // An offglide already sits near the consonant's target, so the closure is quick.
        let offglide = lp.spec().f_end.is_some() || (segs[i].coda && dl == 2);
        if offglide && is_obstruent(rp) {
            (ms(CLOSURE_AFTER_OFFGLIDE_MS), inside)
        } else {
            (outside, inside)
        }
    } else if dl == 1 {
        transition_frames(Ph::AX)
    } else {
        (2, 2)
    })
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
    let frames = |ms: f64| ms / FRAME_MS;
    // Hertz, Younes & Zinovieva (1999): AY, AW and OY are vowel plus glide, with
    // a fixed glide transition and a brief offglide; the onset vowel takes the
    // rest. Short ones and the one-phone diphthongs scale proportionally.
    let fixed = matches!(ph, Ph::AY | Ph::AW | Ph::OY)
        && len as f64 >= frames(DIPHTHONG_GLIDE_MS + DIPHTHONG_OFFGLIDE_MS + MIN_ONSET_MS);
    let (glide_start, glide_len) = if fixed {
        let glide = frames(DIPHTHONG_GLIDE_MS);
        (len as f64 - frames(DIPHTHONG_OFFGLIDE_MS) - glide, glide)
    } else {
        (0.25 * len as f64, 0.6 * len as f64)
    };
    (0..len)
        .map(|k| match spec.f_end {
            Some(end) => {
                let t = ((k as f64 - glide_start) / glide_len.max(1.0)).clamp(0.0, 1.0);
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
        let Some((left_frames, right_frames)) = transition_window(segs, i) else {
            continue;
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
    ((prosody::aspiration_ms(segs, i) / FRAME_MS).round() as usize).min(next_len / 2)
}

fn is_voiceless(p: Option<Ph>) -> bool {
    p.is_some_and(|p| !p.is_voiced())
}

/// Per-frame (breathiness, offset) for a sonorant: breathiness 0 is modal and
/// 1 fully breathy; offset rises from 0 to 1 over a vowel ending in silence.
/// Klatt & Klatt (1990) found breathiness rises toward vowel offsets into
/// silence and near voiceless consonants, and is greater in unstressed and
/// sentence-final syllables.
fn breathiness(segs: &[Seg], i: usize, len: usize) -> Vec<(f64, f64)> {
    let s = &segs[i];
    let prev = if i > 0 { segs[i - 1].ph } else { None };
    let next = segs.get(i + 1).and_then(|n| n.ph);
    let before_silence = segs.get(i + 1).is_none_or(|n| n.ph.is_none());
    let frames = |ms: f64| (ms / FRAME_MS).max(1.0);
    let mut floor: f64 = 0.0;
    if s.stress == 0 || s.function_word {
        floor = 0.15;
    }
    if s.phrase_final {
        floor = floor.max(0.25);
    }
    (0..len)
        .map(|k| {
            let from_end = (len - k) as f64;
            let mut b = floor;
            let mut offset = 0.0;
            if before_silence {
                offset = (1.0 - from_end / frames(OFFSET_RAMP_MS)).max(0.0);
                b = b.max(offset);
            } else if is_voiceless(next) {
                b = b.max(VOICELESS_PEAK * (1.0 - from_end / frames(VOICELESS_RAMP_MS)));
            }
            if is_voiceless(prev) {
                b = b.max(VOICELESS_PEAK * (1.0 - (k + 1) as f64 / frames(VOICELESS_RAMP_MS)));
            }
            (b.clamp(0.0, 1.0), offset)
        })
        .collect()
}

/// Applies breathy phonation (Klatt & Klatt 1990, Sec. III B steps 5 and 6):
/// a higher open quotient strengthens the first harmonic, spectral tilt
/// weakens the upper harmonics, aspiration noise replaces them, and glottal
/// losses widen B1 and B2.
fn apply_voice_quality(fr: &mut Frame, b: f64, offset: f64) {
    fr.oq = lerp(MODAL_OQ, BREATHY_OQ, b);
    fr.tl = BREATHY_TL * b;
    fr.b[0] += BREATHY_B1_HZ * b;
    fr.b[1] += BREATHY_B2_HZ * b;
    fr.av -= OFFSET_AV_DROP_DB * offset;
    if b > 0.0 {
        // Noise amplitude grows in proportion to breathiness, so it fades in
        // from silence rather than switching on.
        fr.ah = fr.ah.max(BREATHY_AH + 20.0 * b.log10());
    }
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

    for i in 0..segs.len() {
        if !segs[i].ph.is_some_and(Ph::is_sonorant) {
            continue;
        }
        let b = breathiness(segs, i, lens[i]);
        for (fr, b) in frames[starts[i]..starts[i] + lens[i]].iter_mut().zip(b) {
            if fr.av > 0.0 {
                apply_voice_quality(fr, b.0, b.1);
            }
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
    fn vowel_becomes_breathy_before_silence() {
        let (segs, starts, lens, frames) = frames_for("saw");
        let vowel = segs.iter().position(|s| s.ph == Some(Ph::AO)).unwrap();
        let mid = frames[starts[vowel] + lens[vowel] / 2];
        let last = frames[starts[vowel] + lens[vowel] - 1];
        assert!(last.oq > mid.oq && last.tl > mid.tl && last.ah > mid.ah);
        assert!(last.av > 0.0);
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
