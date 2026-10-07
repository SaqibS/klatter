//! Synthesis by rule: converts timed phoneme segments into Klatt parameter
//! frames, with formant transitions, stop closures, bursts and aspiration.

use crate::klatt::{FRAME_MS, Frame};
use crate::phoneme::{Class, Ph, Place};
use crate::prosody;
use crate::utterance::Seg;

const ASPIRATION_DB: f64 = 55.0;
const VOICE_BAR_DB: f64 = 47.0;
/// Eloquence's /h/ is mostly voiced, with noise on top, rather than pure noise.
const HH_AV_DB: f64 = 52.0;
const HH_AH_DB: f64 = 55.0;
/// Eloquence moves between sounds over about 25 ms; switching voicing or
/// frication within one 5 ms frame sounds like clips spliced together.
const CROSS_FADE_FRAMES: usize = 5;
/// Fades run from this inaudible level so they span only the audible range.
const FADE_FLOOR_DB: f64 = 30.0;
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

/// Vowel timing policy; the default keeps the original duration rules.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Timing {
    #[default]
    Original,
    CompactHolds,
}

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
/// Eloquence reaches vowel targets in about half the time these rules give;
/// with its short, evenly timed vowels, full-length transitions leave vowels
/// drifting toward their neighbours instead of holding their quality.
const TRANSITION_SCALE: f64 = 0.5;

fn scale_transition(frames: usize) -> usize {
    (frames as f64 * TRANSITION_SCALE).round() as usize
}

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

fn transition_windows(segs: &[Seg], lens: &[usize]) -> Vec<(usize, usize)> {
    let mut windows: Vec<(usize, usize)> = (0..segs.len().saturating_sub(1))
        .map(|i| transition_window(segs, i).unwrap_or((0, 0)))
        .map(|(before, after)| (scale_transition(before), scale_transition(after)))
        .collect();
    for i in 0..segs.len() {
        let incoming = if i > 0 { windows[i - 1].1 } else { 0 };
        let outgoing = windows.get(i).map_or(0, |w| w.0);
        let available = lens[i].saturating_sub(1);
        if incoming + outgoing > available {
            // Share only contested time, keeping one target frame between transitions.
            let aspiration = if i > 0 {
                aspiration_frames(segs, i - 1, available)
            } else {
                0
            };
            let incoming = incoming
                .min(available / 2)
                .max(available.saturating_sub(outgoing))
                .max(aspiration);
            if i > 0 {
                windows[i - 1].1 = incoming;
            }
            if let Some(w) = windows.get_mut(i) {
                w.0 = outgoing.min(available - incoming);
            }
        }
    }
    windows
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
    let windows = transition_windows(segs, lens);
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
        // The offglide must finish before the outgoing transition can leave its target.
        let outgoing = if ph.spec().f_end.is_some() {
            windows.get(i).map_or(0, |w| w.0)
        } else {
            0
        };
        let mut t = targets(ph, lens[i] - outgoing);
        t.resize(lens[i], *t.last().unwrap());
        if segs[i].ph == Some(Ph::HH) {
            for f in &mut t {
                f.b = [300.0, 150.0, 200.0];
            }
        }
        track.extend(t);
    }
    let target = track.clone();

    for (i, &(left_frames, right_frames)) in windows.iter().enumerate() {
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
    ms(prosody::aspiration_ms(segs, i)).min(next_len)
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
    build_frames_with_timing(segs, Timing::Original)
}

/// Compact timing resamples holds after the baseline layout is constructed;
/// `Seg::dur_ms` and `frame_layout` continue to describe the original timeline.
pub fn build_frames_with_timing(segs: &[Seg], timing: Timing) -> Vec<Frame> {
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
                    fr.av = HH_AV_DB;
                    fr.ah = HH_AH_DB;
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
                        // Even voiceless closures carry a low murmur so
                        // phrases flow without dead gaps, matching Eloquence.
                        fr.avs = VOICE_BAR_DB;
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

    cross_fade(segs, &starts, &lens, &mut frames);

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
    match timing {
        Timing::Original => frames,
        Timing::CompactHolds => compact_holds(segs, &starts, &lens, &formants, &frames),
    }
}

/// Smooths voicing and fricative noise across segment boundaries. Stop and
/// affricate bursts stay abrupt because their sharpness is the cue.
fn cross_fade(segs: &[Seg], starts: &[usize], lens: &[usize], frames: &mut [Frame]) {
    let mut fricative = vec![false; frames.len()];
    for i in 0..segs.len() {
        if segs[i].ph.is_some_and(|p| p.class() == Class::Fricative) {
            fricative[starts[i]..starts[i] + lens[i]].fill(true);
        }
    }
    let half = CROSS_FADE_FRAMES / 2;
    let smooth = |track: Vec<f64>| -> Vec<f64> {
        (0..track.len())
            .map(|k| {
                let window = &track[k.saturating_sub(half)..(k + half + 1).min(track.len())];
                let mean =
                    window.iter().map(|v| v.max(FADE_FLOOR_DB)).sum::<f64>() / window.len() as f64;
                if mean > FADE_FLOOR_DB + 0.5 {
                    mean
                } else {
                    0.0
                }
            })
            .collect()
    };
    let av = smooth(frames.iter().map(|f| f.av).collect());
    let noise = smooth(
        frames
            .iter()
            .zip(&fricative)
            .map(|(f, &m)| if m { f.af } else { 0.0 })
            .collect(),
    );
    for k in 0..frames.len() {
        frames[k].av = av[k];
        if fricative[k] {
            frames[k].af = noise[k];
        } else if noise[k] > 0.0 && frames[k].af == 0.0 {
            // The fading tail keeps the spectrum of the fricative it belongs to.
            let source = (1..=half)
                .flat_map(|d| [k.checked_sub(d), Some(k + d)])
                .flatten()
                .find(|&j| j < frames.len() && fricative[j])
                .unwrap();
            frames[k].af = noise[k];
            frames[k].ab = frames[source].ab;
            frames[k].a = frames[source].a;
        }
    }
}

fn interpolate_frame(a: Frame, b: Frame, t: f64) -> Frame {
    Frame {
        f0: lerp(a.f0, b.f0, t),
        av: lerp(a.av, b.av, t),
        oq: lerp(a.oq, b.oq, t),
        tl: lerp(a.tl, b.tl, t),
        di: lerp(a.di, b.di, t),
        avs: lerp(a.avs, b.avs, t),
        ah: lerp(a.ah, b.ah, t),
        af: lerp(a.af, b.af, t),
        ab: lerp(a.ab, b.ab, t),
        f: std::array::from_fn(|j| lerp(a.f[j], b.f[j], t)),
        b: std::array::from_fn(|j| lerp(a.b[j], b.b[j], t)),
        df1: lerp(a.df1, b.df1, t),
        db1: lerp(a.db1, b.db1, t),
        fnz: lerp(a.fnz, b.fnz, t),
        ftp: lerp(a.ftp, b.ftp, t),
        btp: lerp(a.btp, b.btp, t),
        ftz: lerp(a.ftz, b.ftz, t),
        btz: lerp(a.btz, b.btz, t),
        a: std::array::from_fn(|j| lerp(a.a[j], b.a[j], t)),
    }
}

fn longest_hold(formants: &[Formants], start: usize, end: usize) -> std::ops::Range<usize> {
    let mut longest = start..start;
    let mut begin = start;
    for k in start..end {
        if k + 1 == end || formants[k].f != formants[k + 1].f || formants[k].b != formants[k + 1].b
        {
            if k + 1 - begin > longest.len() {
                longest = begin..k + 1;
            }
            begin = k + 1;
        }
    }
    longest
}

fn retained_hold_frames(hold: usize, vowel: usize, floor: usize) -> usize {
    // Only excess dwell time is elastic; short targets and contrastive vowel
    // minima must survive.
    let knee = ms(20.0);
    let excess = hold.saturating_sub(knee);
    let remove = ((excess as f64 * 0.75).round() as usize).min(vowel.saturating_sub(floor));
    hold - remove
}

fn hold_sampling_positions(frames: &[Frame], retained: usize) -> Vec<f64> {
    if retained == frames.len() {
        return (0..frames.len()).map(|k| k as f64).collect();
    }
    // A continuous time map can still turn a gentle pitch fall into a rapid
    // one. Spend more of the retained time where pitch is moving quickly.
    let max_step = frames
        .windows(2)
        .map(|w| (w[1].f0 - w[0].f0).abs())
        .fold(0.0, f64::max);
    let rate_limit = max_step.max(3.0);
    let base = (retained - 1) as f64 / (frames.len() - 1) as f64;
    let mut clock = vec![0.0];
    for pair in frames.windows(2) {
        clock.push(clock.last().unwrap() + base.max((pair[1].f0 - pair[0].f0).abs() / rate_limit));
    }
    let total = *clock.last().unwrap();
    let count = (total.ceil() as usize + 1).max(retained).min(frames.len());
    let mut left = 0;
    (0..count)
        .map(|k| {
            if k == 0 {
                return 0.0;
            }
            if k + 1 == count {
                return (frames.len() - 1) as f64;
            }
            let time = k as f64 * total / (count - 1) as f64;
            while clock[left + 1] < time {
                left += 1;
            }
            left as f64 + (time - clock[left]) / (clock[left + 1] - clock[left])
        })
        .collect()
}

fn compact_holds(
    segs: &[Seg],
    starts: &[usize],
    lens: &[usize],
    formants: &[Formants],
    frames: &[Frame],
) -> Vec<Frame> {
    let mut result = Vec::with_capacity(frames.len());
    for (i, seg) in segs.iter().enumerate() {
        let start = starts[i];
        let end = start + lens[i];
        let Some(ph) = seg.ph.filter(|ph| ph.is_vowel()) else {
            result.extend_from_slice(&frames[start..end]);
            continue;
        };
        let aspiration = if i > 0 {
            aspiration_frames(segs, i - 1, lens[i])
        } else {
            0
        };
        let hold = longest_hold(formants, start + aspiration, end);
        let stressed = seg.stress > 0 && !seg.function_word;
        let minimum = ph.durations().1 / if stressed { 1.0 } else { 2.0 };
        let floor = aspiration + (minimum / FRAME_MS).ceil() as usize;
        let positions = hold_sampling_positions(
            &frames[hold.clone()],
            retained_hold_frames(hold.len(), lens[i], floor),
        );
        let retained = positions.len();
        if retained == hold.len() {
            result.extend_from_slice(&frames[start..end]);
            continue;
        }
        result.extend_from_slice(&frames[start..hold.start]);
        // Sampling the whole hold keeps its endpoints and avoids a jump in F0
        // or source envelopes even when only the formants are stationary.
        for (k, &position) in positions.iter().enumerate() {
            if k == 0 {
                result.push(frames[hold.start]);
            } else if k + 1 == retained {
                result.push(frames[hold.end - 1]);
            } else {
                let left = position.floor() as usize;
                let a = frames[hold.start + left];
                let b = frames[hold.start + left + 1];
                result.push(interpolate_frame(a, b, position - left as f64));
            }
        }
        result.extend_from_slice(&frames[hold.end..end]);
    }
    result
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
    fn voicing_fades_into_stop_closure_but_burst_stays_sharp() {
        let (segs, starts, _, frames) = frames_for("at");
        let t = segs.iter().position(|s| s.ph == Some(Ph::T)).unwrap();
        let closure = starts[t];
        for k in closure - 2..closure + 3 {
            let step = frames[k - 1].av.max(FADE_FLOOR_DB) - frames[k].av.max(FADE_FLOOR_DB);
            assert!(
                step < 15.0,
                "frame {k}: {} -> {}",
                frames[k - 1].av,
                frames[k].av
            );
        }
        let burst = frames[closure..].iter().position(|f| f.af > 0.0).unwrap() + closure;
        assert_eq!(frames[burst - 1].af, 0.0);
    }

    #[test]
    fn layout_covers_all_frames() {
        let (_, starts, lens, frames) = frames_for("Hello there, how are you?");
        assert_eq!(starts.last().unwrap() + lens.last().unwrap(), frames.len());
    }

    #[test]
    fn compact_holds_preserve_other_segments_and_moving_transitions() {
        for text in [
            "Alice was beginning to get very tired.",
            "The bank is on the other side. Keep that in mind.",
            "Way down in the well, the bookshelves were empty.",
            "The rain moved slowly over the hills.",
            "Please put the blue suitcase beside the door.",
            "Five loud boys try tea. A quick voice. Who?",
        ] {
            let (segs, starts, lens, baseline) = frames_for(text);
            assert_eq!(baseline, build_frames_with_timing(&segs, Timing::Original));
            let tracks = formant_tracks(&segs, &starts, &lens);
            let compact = build_frames_with_timing(&segs, Timing::CompactHolds);
            let mut offset = 0;
            for (i, seg) in segs.iter().enumerate() {
                let start = starts[i];
                let end = start + lens[i];
                let Some(ph) = seg.ph.filter(|ph| ph.is_vowel()) else {
                    assert_eq!(
                        &compact[offset..offset + lens[i]],
                        &baseline[start..end],
                        "{text}: {i}"
                    );
                    offset += lens[i];
                    continue;
                };
                let aspiration = if i > 0 {
                    aspiration_frames(&segs, i - 1, lens[i])
                } else {
                    0
                };
                let hold = longest_hold(&tracks, start + aspiration, end);
                let min = ph.durations().1
                    / if seg.stress > 0 && !seg.function_word {
                        1.0
                    } else {
                        2.0
                    };
                let floor = aspiration + (min / FRAME_MS).ceil() as usize;
                let retained = hold_sampling_positions(
                    &baseline[hold.clone()],
                    retained_hold_frames(hold.len(), lens[i], floor),
                )
                .len();
                let new_len = lens[i] - hold.len() + retained;
                assert!(new_len >= floor.min(lens[i]), "{text}: {ph:?}");
                let actual = &compact[offset..offset + new_len];
                let prefix = hold.start - start;
                assert_eq!(&actual[..prefix], &baseline[start..hold.start]);
                assert_eq!(&actual[prefix + retained..], &baseline[hold.end..end]);
                assert_eq!(actual.first(), baseline[start..end].first());
                assert_eq!(actual.last(), baseline[start..end].last());
                if retained > 0 {
                    assert_eq!(actual[prefix], baseline[hold.start]);
                    assert_eq!(actual[prefix + retained - 1], baseline[hold.end - 1]);
                }
                offset += new_len;
            }
            assert_eq!(offset, compact.len());
            assert!(compact.len() < baseline.len(), "{text}");
            assert!(compact.iter().all(|f| f.f0.is_finite()
                && f.av.is_finite()
                && f.f.iter().chain(&f.b).all(|v| v.is_finite())));
        }
    }

    #[test]
    fn elastic_hold_rule_is_monotonic_and_does_not_lengthen_short_vowels() {
        for floor in [0, 10, 20, 30, 50] {
            let mut previous = 0;
            for hold in 0..200 {
                let vowel = hold + 10;
                let retained = retained_hold_frames(hold, vowel, floor);
                let length = vowel - hold + retained;
                assert!(retained <= hold);
                assert!(length >= previous);
                assert!(length >= vowel.min(floor));
                if hold <= ms(20.0) {
                    assert_eq!(retained, hold);
                }
                previous = length;
            }
        }
    }

    #[test]
    fn compact_pitch_safeguard_preserves_fast_contours_without_new_large_steps() {
        for step in [0.0, 0.5, 1.5, 3.0, 4.0] {
            let frames: Vec<_> = (0..40)
                .map(|i| Frame {
                    f0: 250.0 - i as f64 * step,
                    ..Frame::default()
                })
                .collect();
            let positions = hold_sampling_positions(&frames, 10);
            let retained = positions.len();
            let actual_step = 39.0 * step / (retained - 1) as f64;
            assert!(actual_step <= step.max(3.0) + 1e-12);
            assert!((10..=40).contains(&retained));
            if step >= 3.0 {
                assert_eq!(retained, frames.len());
            }
        }
        let frames: Vec<_> = (0..40)
            .map(|i| Frame {
                f0: 140.0 - (i as f64 - 20.0).max(0.0) * 2.5,
                ..Frame::default()
            })
            .collect();
        let positions = hold_sampling_positions(&frames, 10);
        assert!(
            positions.len() <= 25,
            "a local fall must not prevent shortening the flat part"
        );
        assert_eq!(positions[0], 0.0);
        assert_eq!(*positions.last().unwrap(), 39.0);
        let pitch: Vec<_> = positions
            .iter()
            .map(|&p| {
                let left = p.floor() as usize;
                lerp(
                    frames[left].f0,
                    frames[(left + 1).min(39)].f0,
                    p - left as f64,
                )
            })
            .collect();
        assert!(positions.windows(2).all(|w| w[1] > w[0]));
        assert!(pitch.windows(2).all(|w| (w[1] - w[0]).abs() <= 3.0 + 1e-12));
    }

    #[test]
    fn compact_holds_resample_source_envelopes_instead_of_splicing_them() {
        let (mut segs, _, _, _) = frames_for("saw");
        let vowel = segs.iter().position(|s| s.is_vowel()).unwrap();
        segs[vowel].dur_ms = 300.0;
        let (starts, lens) = frame_layout(&segs);
        let old = build_frames(&segs);
        let new = build_frames_with_timing(&segs, Timing::CompactHolds);
        let removed = old.len() - new.len();
        assert!(removed >= ms(100.0));
        let a = &old[starts[vowel]..starts[vowel] + lens[vowel]];
        let b = &new[starts[vowel]..starts[vowel] + lens[vowel] - removed];
        assert_eq!(a.first(), b.first());
        assert_eq!(a.last(), b.last());
        assert!(a.iter().any(|f| f.oq != a[0].oq));
        let low = a.iter().map(|f| f.oq).fold(f64::INFINITY, f64::min);
        let high = a.iter().map(|f| f.oq).fold(f64::NEG_INFINITY, f64::max);
        assert!(b.iter().all(|f| f.oq >= low && f.oq <= high));
        let tracks = formant_tracks(&segs, &starts, &lens);
        let hold = longest_hold(&tracks, starts[vowel], starts[vowel] + lens[vowel]);
        let retained = hold.len() - removed;
        let max_step = old[hold.clone()]
            .windows(2)
            .map(|w| (w[1].oq - w[0].oq).abs())
            .fold(0.0, f64::max);
        let positions = hold_sampling_positions(
            &old[hold.clone()],
            retained_hold_frames(hold.len(), lens[vowel], ms(Ph::AO.durations().1)),
        );
        let resampling_step = positions
            .windows(2)
            .map(|w| w[1] - w[0])
            .fold(0.0, f64::max);
        let local = hold.start - starts[vowel];
        assert!(
            b[local..local + retained]
                .windows(2)
                .all(|w| (w[1].oq - w[0].oq).abs() <= max_step * resampling_step + 1e-9)
        );
    }

    #[test]
    fn compact_holds_do_not_shorten_diphthong_glides() {
        for (text, ph) in [("five", Ph::AY), ("loud", Ph::AW), ("voice", Ph::OY)] {
            let (segs, starts, lens, old) = frames_for(text);
            let vowel = segs.iter().position(|s| s.ph == Some(ph)).unwrap();
            let new = build_frames_with_timing(&segs, Timing::CompactHolds);
            let removed = old.len() - new.len();
            assert!(removed >= ms(30.0));
            let closure =
                starts[vowel] + lens[vowel] - scale_transition(ms(CLOSURE_AFTER_OFFGLIDE_MS));
            let glide = closure - ms(DIPHTHONG_OFFGLIDE_MS + DIPHTHONG_GLIDE_MS);
            assert_eq!(
                &old[glide..closure],
                &new[glide - removed..closure - removed],
                "{text}"
            );
        }
    }

    #[test]
    fn compact_empty_and_tiny_segments_remain_valid() {
        assert!(build_frames_with_timing(&[], Timing::CompactHolds).is_empty());
        let (mut segs, _, _, _) = frames_for("try a quick voice");
        for duration in [5.0, 10.0, 25.0, 60.0] {
            for seg in &mut segs {
                seg.dur_ms = duration;
            }
            let old = build_frames(&segs);
            let new = build_frames_with_timing(&segs, Timing::CompactHolds);
            assert!(new.len() <= old.len());
            let audio = crate::klatt::Synth::new(22050).render(&new);
            assert!(audio.iter().all(|s| s.is_finite()));
        }
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
    fn aspiration_matches_the_prosodic_budget() {
        for text in ["tea", "tree", "try", "quick", "to be", "key", "stay", "dee"] {
            let (segs, starts, lens, frames) = frames_for(text);
            for i in 0..segs.len() - 1 {
                if !segs[i].ph.is_some_and(Ph::is_plosive) {
                    continue;
                }
                let next = &frames[starts[i + 1]..starts[i + 1] + lens[i + 1]];
                let actual = next
                    .iter()
                    .take_while(|f| f.av == 0.0 && f.ah > 0.0)
                    .count();
                let expected = ms(prosody::aspiration_ms(&segs, i));
                assert_eq!(actual, expected, "{text}: {:?}", segs[i].ph);
            }
        }
    }

    #[test]
    fn release_transition_can_use_more_than_half_the_segment() {
        let (mut segs, _, _, _) = frames_for("tea");
        let vowel = segs.iter().position(|s| s.ph == Some(Ph::IY)).unwrap();
        segs[vowel].dur_ms = 50.0;
        let (starts, lens) = frame_layout(&segs);
        let frames = build_frames(&segs);
        let start = starts[vowel];
        let release = scale_transition(ms(RELEASE_AFTER_ASPIRATION_MS));
        assert!(release > lens[vowel] / 2);
        let target = Ph::IY.spec().f;
        assert_ne!(frames[start + release - 1].f, target);
        assert_eq!(frames[start + release].f, target);
    }

    #[test]
    fn short_sonorants_keep_aspiration_and_a_voiced_target() {
        let (mut segs, _, _, _) = frames_for("try");
        for s in &mut segs {
            s.stress = 0;
            s.function_word = true;
        }
        assign_durations(&mut segs);
        let (starts, lens) = frame_layout(&segs);
        let windows = transition_windows(&segs, &lens);
        let frames = build_frames(&segs);
        let r = segs.iter().position(|s| s.ph == Some(Ph::R)).unwrap();
        let aspiration = ms(prosody::aspiration_ms(&segs, r - 1));
        assert_eq!(aspiration, 5);
        assert!(windows[r - 1].1 >= aspiration);
        assert!(windows[r - 1].1 + windows[r].0 < lens[r]);
        let segment = &frames[starts[r]..starts[r] + lens[r]];
        assert!(
            segment[..aspiration]
                .iter()
                .all(|f| f.av == 0.0 && f.ah > 0.0)
        );
        assert!(segment[aspiration..].iter().all(|f| f.av > 0.0));
        assert_eq!(segment[windows[r - 1].1].f, Ph::R.spec().f);
    }

    #[test]
    fn short_transition_windows_do_not_overlap_or_escape_segments() {
        let (mut segs, _, _, _) = frames_for("quick five loud voice");
        for duration in [5.0, 10.0, 25.0, 60.0, 120.0] {
            for s in &mut segs {
                s.dur_ms = duration;
            }
            let (_, lens) = frame_layout(&segs);
            let windows = transition_windows(&segs, &lens);
            for i in 0..segs.len() {
                let incoming = if i > 0 { windows[i - 1].1 } else { 0 };
                let outgoing = windows.get(i).map_or(0, |w| w.0);
                assert!(incoming + outgoing < lens[i], "{duration} ms: segment {i}");
            }
            let frames = build_frames(&segs);
            assert_eq!(frames.len(), lens.iter().sum::<usize>());
            assert!(
                frames
                    .iter()
                    .all(|f| f.f0.is_finite() && f.f.iter().all(|v| v.is_finite()))
            );
        }
    }

    #[test]
    fn diphthongs_reach_and_hold_offglides_before_closure() {
        for (text, ph) in [("five", Ph::AY), ("loud", Ph::AW), ("voice", Ph::OY)] {
            let (segs, starts, lens, frames) = frames_for(text);
            let vowel = segs.iter().position(|s| s.ph == Some(ph)).unwrap();
            let closure =
                starts[vowel] + lens[vowel] - scale_transition(ms(CLOSURE_AFTER_OFFGLIDE_MS));
            let offglide = closure - ms(DIPHTHONG_OFFGLIDE_MS);
            let glide = offglide - ms(DIPHTHONG_GLIDE_MS);
            let spec = ph.spec();
            let target = spec.f_end.unwrap();
            assert_eq!(frames[glide].f, spec.f, "{text}");
            for (k, fr) in frames[glide..offglide].iter().enumerate() {
                let expected: [f64; 3] = std::array::from_fn(|j| {
                    lerp(
                        spec.f[j],
                        target[j],
                        k as f64 / ms(DIPHTHONG_GLIDE_MS) as f64,
                    )
                });
                assert_eq!(fr.f, expected, "{text}: glide frame {k}");
            }
            for fr in &frames[offglide..closure] {
                assert_eq!(fr.f, target, "{text}");
            }
            assert_ne!(frames[closure].f, target, "{text}");
        }
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
