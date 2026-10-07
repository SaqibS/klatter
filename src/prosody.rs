//! Prosody: Klatt's (1979) segmental duration rules, as summarized in Klatt
//! (1987), and a hat-pattern F0 contour with declination and accent peaks.

use crate::phoneme::{Class, Ph, Place};
use crate::text::Break;
use crate::utterance::Seg;

pub fn pause_ms(brk: Option<Break>, utterance_edge: bool) -> f64 {
    match brk {
        _ if utterance_edge => 150.0,
        // Eloquence's reference reading barely pauses; boundaries are carried
        // mainly by pitch and final lengthening.
        Some(Break::Comma) => 100.0,
        Some(_) => 200.0,
        None => 150.0,
    }
}

fn neighbor(segs: &[Seg], i: usize, offset: isize) -> Option<Ph> {
    let j = i as isize + offset;
    if j < 0 {
        return None;
    }
    segs.get(j as usize).and_then(|s| s.ph)
}

/// Rule 9 (postvocalic context): effect of the consonant after a vowel.
fn postvocalic_factor(segs: &[Seg], i: usize) -> f64 {
    let s = &segs[i];
    if s.word_final {
        return 1.2;
    }
    // Sonorant codas pass the effect on from the obstruent after them.
    let mut j = i + 1;
    while j < segs.len() && !segs[j - 1].word_final {
        let Some(p) = segs[j].ph else { break };
        if p.is_vowel() {
            return 1.0;
        }
        if matches!(p.class(), Class::Liquid | Class::Glide) && !segs[j].word_final {
            j += 1;
            continue;
        }
        return match (p.class(), p.is_voiced()) {
            (Class::Fricative, true) => 1.6,
            (Class::Stop | Class::Affricate, true) => 1.2,
            (Class::Nasal, _) => 0.85,
            (Class::Stop | Class::Affricate, false) => 0.7,
            _ => 1.0,
        };
    }
    1.0
}

/// Exclusive end of the acoustic nucleus starting at vowel `i`: the vowel plus
/// following liquids and glides in the same syllable (Hertz & Huffman 1992).
pub fn nucleus_end(segs: &[Seg], i: usize) -> usize {
    let mut j = i + 1;
    while j < segs.len()
        && !segs[j - 1].word_final
        && segs[j].coda
        && segs[j]
            .ph
            .is_some_and(|p| matches!(p.class(), Class::Liquid | Class::Glide))
    {
        j += 1;
    }
    j
}

/// Aspiration after the plosive at `i`, or 0 if it isn't aspirated.
pub fn aspiration_ms(segs: &[Seg], i: usize) -> f64 {
    let s = &segs[i];
    let Some(ph) = s.ph else { return 0.0 };
    let after_s = i > 0 && segs[i - 1].ph == Some(Ph::S) && !s.word_initial;
    let Some(next) = segs
        .get(i + 1)
        .filter(|n| n.ph.is_some_and(Ph::is_sonorant))
    else {
        return 0.0;
    };
    if !ph.is_plosive() || ph.is_voiced() || after_s {
        return 0.0;
    }
    let ms = if next.stress > 0 && !next.function_word {
        50.0
    } else {
        25.0
    };
    if ph.place() == Place::Velar {
        ms + 10.0
    } else {
        ms
    }
}

/// Hertz & Huffman (1992): the voiced vowel after aspiration is about 20 ms
/// shorter than elsewhere, but the aspiration itself adds to the syllable.
const VOWEL_SHORTENING_AFTER_ASPIRATION_MS: f64 = 20.0;

/// Eloquence keeps syllables evenly timed: stressed vowels are shorter than
/// Klatt's rules give, which avoids a drawl on content words, while unstressed
/// syllables of content words stay audible instead of being swallowed.
const STRESSED_VOWEL_FACTOR: f64 = 0.65;
const UNSTRESSED_CONTENT_VOWEL_FACTOR: f64 = 1.4;

/// Eloquence's stop closures and fricatives are about 20% shorter than
/// Klatt's, which keeps consonants crisp and the rhythm vowel-led.
const OBSTRUENT_FACTOR: f64 = 0.8;

/// Klatt's value is 1.4, but final vowels already escape non-final shortening
/// and get the full postvocalic effect, so 1.4 drawls them; Eloquence marks
/// boundaries with only mild lengthening.
const PHRASE_FINAL_LENGTHENING: f64 = 1.0;

pub fn assign_durations(segs: &mut [Seg]) {
    let last = segs.len().saturating_sub(1);
    // The vowel's (percent, minimum), kept so its nucleus can be timed as a whole.
    let mut vowel_scaling = vec![(1.0, 0.0); segs.len()];
    for i in 0..segs.len() {
        let Some(ph) = segs[i].ph else {
            segs[i].dur_ms = pause_ms(segs[i].brk, i == 0 || i == last);
            continue;
        };
        let s = &segs[i];
        let (inh, mut min) = ph.durations();
        let mut prcnt = 1.0;
        let vowel = ph.is_vowel();
        let stressed = s.stress > 0 && !s.function_word;

        // Rule 2: clause-final lengthening of the final syllable's nucleus and coda.
        if s.phrase_final && (vowel || s.coda) {
            prcnt *= PHRASE_FINAL_LENGTHENING;
        }
        // Rule 3: non-phrase-final shortening.
        if vowel && !s.phrase_final {
            prcnt *= 0.6;
        }
        // Rule 4: non-word-final shortening.
        if vowel && s.syllable + 1 < s.syllables_in_word {
            prcnt *= 0.85;
        }
        // Rule 5: polysyllabic shortening.
        if vowel && s.syllables_in_word > 1 {
            prcnt *= 0.8;
        }
        // Rule 6: non-initial consonant shortening.
        if !vowel && !s.word_initial {
            prcnt *= 0.85;
        }
        // Rule 7: unstressed shortening.
        let prev = neighbor(segs, i, -1);
        let next = neighbor(segs, i, 1);
        if !stressed {
            min /= 2.0;
            let medial = s.syllable > 0 && s.syllable + 1 < s.syllables_in_word;
            let prevocalic_approximant = matches!(ph.class(), Class::Liquid | Class::Glide)
                && !s.coda
                && next.is_some_and(Ph::is_vowel);
            prcnt *= if vowel && medial {
                0.5
            } else if prevocalic_approximant {
                0.1
            } else {
                0.7
            };
            if vowel && !s.function_word {
                prcnt *= UNSTRESSED_CONTENT_VOWEL_FACTOR;
            }
        }
        if vowel && stressed {
            prcnt *= STRESSED_VOWEL_FACTOR;
        }
        // Rule 9: postvocalic context, strongest at phrase ends.
        if vowel {
            let f = postvocalic_factor(segs, i);
            prcnt *= if s.phrase_final { f } else { 0.7 + 0.3 * f };
        }
        // Rule 10: shortening in clusters, except phrase-final codas.
        let is_cons = |p: Option<Ph>| p.is_some_and(|p| !p.is_vowel());
        if vowel {
            if next.is_some_and(Ph::is_vowel) {
                prcnt *= 1.2;
            }
            if prev.is_some_and(Ph::is_vowel) {
                prcnt *= 0.7;
            }
        } else if s.phrase_final && s.coda {
            // Phrase-final codas keep their final lengthening.
        } else if is_cons(prev) && is_cons(next) {
            prcnt *= 0.5;
        } else if is_cons(next) || is_cons(prev) {
            prcnt *= 0.7;
        }

        vowel_scaling[i] = (prcnt, min);
        segs[i].dur_ms = (inh - min) * prcnt + min;
        if matches!(
            ph.class(),
            Class::Stop | Class::Fricative | Class::Affricate
        ) {
            segs[i].dur_ms *= OBSTRUENT_FACTOR;
        }
    }

    // The vowel's context scales its whole nucleus; coda liquids and glides
    // keep their own durations and the vowel takes the remainder, so vowel and
    // coda trade time (Hertz & Huffman 1992).
    for i in 0..segs.len() {
        if !segs[i].is_vowel() {
            continue;
        }
        let end = nucleus_end(segs, i);
        if end > i + 1 {
            let (prcnt, vowel_min) = vowel_scaling[i];
            let stressed = segs[i].stress > 0 && !segs[i].function_word;
            let (mut inh, mut min, mut coda) = (0.0, 0.0, 0.0);
            for s in &segs[i..end] {
                let (h, m) = s.ph.map(Ph::durations).unwrap_or_default();
                inh += h;
                min += if stressed { m } else { m / 2.0 };
            }
            for s in &segs[i + 1..end] {
                coda += s.dur_ms;
            }
            segs[i].dur_ms = ((inh - min) * prcnt + min - coda).max(vowel_min);
        }
    }
    for i in 1..segs.len() {
        let asp = aspiration_ms(segs, i - 1);
        if asp > 0.0 {
            let floor = segs[i].ph.map_or(0.0, |p| p.durations().1 / 2.0);
            segs[i].dur_ms =
                (segs[i].dur_ms - VOWEL_SHORTENING_AFTER_ASPIRATION_MS).max(floor) + asp;
        }
    }
}

const TOP_F0: f64 = 120.0;
pub const BASE_F0: f64 = 93.0;
const FLOOR_F0: f64 = 78.0;
const HAT_RISE: f64 = 12.0;
const ACCENT: f64 = 10.0;
const DECLINATION_PER_SEC: f64 = 10.0;

/// Builds an F0 track (Hz per frame) given each segment's first frame and
/// frame count.
pub fn f0_contour(segs: &[Seg], starts: &[usize], lens: &[usize], frame_ms: f64) -> Vec<f64> {
    let total: usize = lens.iter().sum();
    let mut f0 = vec![BASE_F0; total];
    let mut i = 0;
    let mut sentence_start = 0usize;
    while i < segs.len() {
        if segs[i].ph.is_none() {
            if matches!(
                segs[i].brk,
                Some(Break::Period | Break::Question | Break::Exclamation)
            ) {
                sentence_start = starts[i] + lens[i];
            }
            for k in starts[i]..starts[i] + lens[i] {
                f0[k] = BASE_F0;
            }
            i += 1;
            continue;
        }
        let clause_begin = i;
        while i < segs.len() && segs[i].ph.is_some() {
            i += 1;
        }
        let ending = segs.get(i).and_then(|s| s.brk).unwrap_or(Break::Period);
        clause_contour(
            &segs[clause_begin..i],
            &starts[clause_begin..i],
            &lens[clause_begin..i],
            ending,
            sentence_start,
            frame_ms,
            &mut f0,
        );
    }
    smooth(&mut f0, 9);
    add_flutter(&mut f0, frame_ms);
    f0
}

fn clause_contour(
    segs: &[Seg],
    starts: &[usize],
    lens: &[usize],
    ending: Break,
    sentence_start: usize,
    frame_ms: f64,
    f0: &mut [f64],
) {
    let begin = starts[0];
    let end = starts[segs.len() - 1] + lens[segs.len() - 1];
    let accents: Vec<usize> = (0..segs.len())
        .filter(|&k| segs[k].is_vowel() && segs[k].stress == 1 && !segs[k].function_word)
        .collect();
    let baseline = |frame: usize| {
        let secs = (frame - sentence_start) as f64 * frame_ms / 1000.0;
        (TOP_F0 - DECLINATION_PER_SEC * secs).max(BASE_F0)
    };
    let hat_on = accents.first().map(|&k| starts[k]).unwrap_or(end);
    // Hertz, Younes & Zinovieva (1999) put the accent's high target about 75% of
    // the way through the nucleus, rising from the middle of the onset.
    let peak_of = |k: usize| {
        let e = nucleus_end(segs, k);
        starts[k] + (starts[e - 1] + lens[e - 1] - starts[k]) * 3 / 4
    };
    let last_accent = accents.last().copied();
    let hat_off = last_accent.map(peak_of).unwrap_or(end);
    let rise = (100.0 / frame_ms) as usize;
    for frame in begin..end {
        let mut v = baseline(frame);
        if frame >= hat_on && frame < hat_off {
            v += HAT_RISE * ((frame - hat_on) as f64 / rise as f64).min(1.0);
        }
        f0[frame] = v;
    }
    for &k in &accents {
        let peak = peak_of(k);
        let half = lens[k].max(8);
        for frame in peak.saturating_sub(half)..(peak + half).min(end) {
            let w = 1.0 - (frame as f64 - peak as f64).abs() / half as f64;
            f0[frame] += ACCENT * w.max(0.0);
        }
    }
    // Boundary tones also apply to clauses with no pitch accents.
    let tail_start = last_accent.map(peak_of).unwrap_or_else(|| {
        segs.iter()
            .rposition(Seg::is_vowel)
            .map_or(begin, |k| starts[k])
    });
    let tail_len = (end - tail_start).max(1) as f64;
    let top = if tail_start > begin {
        f0[tail_start - 1]
    } else {
        baseline(begin)
    };
    for frame in tail_start..end {
        let t = (frame - tail_start) as f64 / tail_len;
        f0[frame] = match ending {
            Break::Period | Break::Exclamation => top + (FLOOR_F0 - top) * t.sqrt(),
            Break::Comma => {
                let dip = baseline(frame) - 5.0;
                if t < 0.6 {
                    top + (dip - top) * (t / 0.6)
                } else {
                    dip + 12.0 * (t - 0.6) / 0.4
                }
            }
            Break::Question => {
                let low = baseline(frame) - 5.0;
                if t < 0.4 {
                    top + (low - top) * (t / 0.4)
                } else {
                    low + 35.0 * ((t - 0.4) / 0.6).powf(1.5)
                }
            }
        };
    }
}

fn smooth(f0: &mut [f64], width: usize) {
    let src = f0.to_vec();
    let half = width / 2;
    for i in 0..f0.len() {
        let lo = i.saturating_sub(half);
        let hi = (i + half + 1).min(src.len());
        f0[i] = src[lo..hi].iter().sum::<f64>() / (hi - lo) as f64;
    }
}

/// Slow quasi-random F0 drift (KLSYN88 "flutter") so steady vowels don't sound
/// mechanical. Klatt & Klatt (1990) found FL = 25% sounds realistic.
fn add_flutter(f0: &mut [f64], frame_ms: f64) {
    use std::f64::consts::TAU;
    const FL: f64 = 25.0;
    for (i, v) in f0.iter_mut().enumerate() {
        let t = i as f64 * frame_ms / 1000.0;
        let wobble = (TAU * 12.7 * t).sin() + (TAU * 7.1 * t).sin() + (TAU * 4.7 * t).sin();
        *v += (FL / 50.0) * (*v / 100.0) * wobble;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::normalize;
    use crate::utterance::build;

    fn durations(text: &str) -> Vec<(Ph, f64)> {
        let mut segs = build(&normalize(text));
        assign_durations(&mut segs);
        segs.iter()
            .filter_map(|s| s.ph.map(|p| (p, s.dur_ms)))
            .collect()
    }

    #[test]
    fn phrase_final_vowel_is_longer() {
        let d = durations("cat cat");
        let vowels: Vec<f64> = d
            .iter()
            .filter(|(p, _)| p.is_vowel())
            .map(|x| x.1)
            .collect();
        assert!(vowels[1] > vowels[0] * 1.05, "{vowels:?}");
    }

    #[test]
    fn vowel_before_voiced_consonant_is_longer() {
        let bead = durations("bead")[1].1;
        let beat = durations("beat")[1].1;
        assert!(bead > beat, "bead {bead} beat {beat}");
    }

    #[test]
    fn coda_liquid_takes_time_from_vowel() {
        let bee = durations("bee");
        let beer = durations("beer");
        let vowel = |d: &[(Ph, f64)]| d.iter().find(|(p, _)| p.is_vowel()).unwrap().1;
        let nucleus: f64 = beer[1..].iter().map(|x| x.1).sum();
        assert!(vowel(&beer) < vowel(&bee), "{beer:?} {bee:?}");
        assert!(nucleus > vowel(&bee), "{beer:?} {bee:?}");
    }

    #[test]
    fn aspiration_adds_to_syllable_but_shortens_voicing() {
        let tea = durations("tea")[1].1;
        let dee = durations("dee")[1].1;
        assert!(tea > dee && tea - 50.0 < dee, "tea {tea} dee {dee}");
    }

    #[test]
    fn aspiration_does_not_consume_the_voiced_minimum() {
        let mut segs = build(&normalize("try"));
        for s in &mut segs {
            s.stress = 0;
            s.function_word = true;
        }
        assign_durations(&mut segs);
        let r = segs.iter().position(|s| s.ph == Some(Ph::R)).unwrap();
        let asp = aspiration_ms(&segs, r - 1);
        assert_eq!(asp, 25.0);
        assert_eq!(segs[r].dur_ms, asp + Ph::R.durations().1 / 2.0);
    }

    #[test]
    fn durations_respect_minimums() {
        for (p, d) in durations("the quick brown fox jumps over the lazy dog") {
            assert!(d >= p.durations().1 / 2.0 - 1e-9, "{p:?} {d}");
        }
    }

    #[test]
    fn consonant_between_consonants_gets_cluster_shortening() {
        let d = durations("extra cat");
        let s = d.iter().find(|(p, _)| *p == Ph::S).unwrap().1;
        let (inh, min) = Ph::S.durations();
        let expected = ((inh - min) * 0.85 * 0.5 + min) * OBSTRUENT_FACTOR;
        assert!((s - expected).abs() < 1e-9, "{s} vs {expected}");
    }

    #[test]
    fn cluster_shortening_respects_pauses_and_final_codas() {
        let last_t = |text: &str| {
            durations(text)
                .into_iter()
                .rev()
                .find(|(p, _)| *p == Ph::T)
                .unwrap()
                .1
        };
        let (inh, min) = Ph::T.durations();
        let final_t = ((inh - min) * PHRASE_FINAL_LENGTHENING * 0.85 + min) * OBSTRUENT_FACTOR;
        assert!((last_t("act") - final_t).abs() < 1e-9);
        assert!((last_t("act. cat") - final_t).abs() < 1e-9);
        let first_t = |text: &str| {
            durations(text)
                .into_iter()
                .find(|(p, _)| *p == Ph::T)
                .unwrap()
                .1
        };
        assert_eq!(first_t("tea"), first_t("bus. tea"));
        assert!(first_t("bus tea") < first_t("bus. tea"));
    }

    #[test]
    fn question_rises_and_statement_falls() {
        for (text, rises) in [
            ("is it raining?", true),
            ("it is raining.", false),
            ("Are you?", true),
            ("Are you.", false),
            ("Who?", true),
            ("Who.", false),
        ] {
            let mut segs = build(&normalize(text));
            assign_durations(&mut segs);
            let lens: Vec<usize> = segs
                .iter()
                .map(|s| (s.dur_ms / 5.0).round().max(1.0) as usize)
                .collect();
            let mut starts = vec![0; lens.len()];
            for k in 1..lens.len() {
                starts[k] = starts[k - 1] + lens[k - 1];
            }
            let f0 = f0_contour(&segs, &starts, &lens, 5.0);
            let last = segs.len() - 2;
            let end = starts[last] + lens[last] - 3;
            let mid = starts[last] + lens[last] / 4;
            assert_eq!(
                f0[end] > f0[mid],
                rises,
                "{text}: {} -> {}",
                f0[mid],
                f0[end]
            );
        }
    }
}
