//! Phone-and-transition timing (Hertz 1991, Journal of Phonetics 19:91-109).
//!
//! Formant transitions are timed units of their own between phones, rather
//! than parts of the phones (pp. 94-95). Each transition's duration depends on
//! the two phones it joins; phones are the steady states between transitions.
//! Klatt's duration rules give each segment its conventional span (Klatt's
//! segmentation puts the whole transition between a vowel and an obstruent,
//! and any aspiration, inside the vowel; Hertz 1991, pp. 94 and 97), so a
//! phone's steady state is its span minus the transition time inside it.
//! Within a syllable nucleus the vowel pays for the transitions, so vowel and
//! transitions trade time (Hertz & Huffman 1992, p. 1173). The spans stay the
//! frame of reference for voicing, frication, bursts and aspiration, which
//! Hertz, Younes & Zinovieva (1999, p. 2285) align with phone and transition
//! edges in the same way.

use crate::klatt::FRAME_MS;
use crate::phoneme::{Class, Ph};
use crate::utterance::Seg;

/// Transition durations (ms) read from Hertz's spectrogram-derived deltas.
/// Stop release into a vowel: [dad] d-a 65 ms (1991, Figs. 1-3, pp. 93-95).
const STOP_RELEASE_MS: f64 = 65.0;
/// Vowel into a stop closure: [dad] and [dat] a-d/t 70 ms (1991, Fig. 1,
/// p. 93; equal before voiced and voiceless stops, p. 94 and Sec. 2.7).
const STOP_CLOSURE_MS: f64 = 70.0;
/// Fricative release into a vowel: [said] s-a 30 ms (1991, Fig. 7, p. 98);
/// five f-a 30 ms (Hertz, Younes & Zinovieva 1999, Figs. 3-4, pp. 2285-2286).
const FRICATIVE_RELEASE_MS: f64 = 30.0;
/// Between two vowels: Said a-i 100 ms, the same as a diphthong's interior
/// transition (1991, Figs. 6-9, pp. 98-100).
const VOWEL_VOWEL_MS: f64 = 100.0;
/// Vowel into a liquid: [eyld] ey-l 80 ms (1991, Figs. 12-13, pp. 102-103).
const VOWEL_LIQUID_MS: f64 = 80.0;
/// Coda liquid into an obstruent: [eyld] l-d 20 ms (1991, Fig. 13, p. 103).
const CODA_SONORANT_CLOSURE_MS: f64 = 20.0;
/// A diphthong's offglide into a consonant: tied y-d 15 ms (Hertz & Huffman
/// 1992, p. 1173).
const OFFGLIDE_CLOSURE_MS: f64 = 15.0;
/// A gliding vowel into a consonant: owed ow-d 40 ms (1991, Fig. 11, p. 102).
const GLIDING_VOWEL_CLOSURE_MS: f64 = 40.0;
/// The papers give no values for these joins, so they keep the durations
/// Klatter used before.
const NASAL_MS: f64 = 40.0;
const VOWEL_FRICATIVE_MS: f64 = 40.0;
const GLIDE_VOWEL_MS: f64 = 100.0;
const ASPIRATE_MS: f64 = 60.0;
const CONSONANT_CONSONANT_MS: f64 = 20.0;
/// A tap is a single ballistic flick of the tongue tip (Ladefoged & Maddieson
/// 1996, ch. 7), so the vowels around it move only briefly:
/// a flap interrupts them for about 35-40 ms in all.
const FLAP_MS: f64 = 20.0;

/// Transitions take about half the time of Hertz's careful
/// single-speaker examples; with short, evenly timed vowels, full-length
/// transitions leave vowels drifting toward their neighbours, and longer
/// natural-rate transitions also dull the voice.
const TRANSITION_SCALE: f64 = 0.5;

/// In fast speech, transitions shorten by `scale^FAST_RATE_EXPONENT` while
/// spans shorten by `scale`. Hertz (1991, p. 106) observes that transitions
/// and steady-state phones are not shortened by the same proportion in fast
/// speech but gives no figures; with this exponent, transitions shrink faster
/// than vowels, which keep their quality at high rates. In slower speech only the steady states stretch:
/// transitions are stable when syllables are lengthened (1991, p. 94).
const FAST_RATE_EXPONENT: f64 = 1.33;

fn is_obstruent(p: Ph) -> bool {
    matches!(p.class(), Class::Stop | Class::Affricate | Class::Fricative)
}

/// Whether the transition out of segment `i` starts from a diphthong's
/// offglide, a gliding vowel's end, or a coda liquid or glide.
fn closure_from_offglide(segs: &[Seg], i: usize) -> Option<f64> {
    let p = segs[i].ph?;
    match p {
        Ph::AY | Ph::AW | Ph::OY => Some(OFFGLIDE_CLOSURE_MS),
        _ if p.spec().f_end.is_some() => Some(GLIDING_VOWEL_CLOSURE_MS),
        _ if segs[i].coda && matches!(p.class(), Class::Liquid | Class::Glide) => {
            Some(CODA_SONORANT_CLOSURE_MS)
        }
        _ => None,
    }
}

/// Duration (ms, natural rate, before scaling) of the transition between
/// segments `i` and `i + 1`, or 0 next to a pause.
fn published_ms(segs: &[Seg], i: usize) -> f64 {
    let (Some(l), Some(r)) = (segs[i].ph, segs[i + 1].ph) else {
        return 0.0;
    };
    let (lc, rc) = (l.class(), r.class());
    let ms = if lc == Class::Aspirate || rc == Class::Aspirate {
        ASPIRATE_MS
    } else if is_obstruent(l) && r.is_sonorant() {
        match lc {
            _ if rc == Class::Nasal => CONSONANT_CONSONANT_MS,
            Class::Fricative => FRICATIVE_RELEASE_MS,
            _ => STOP_RELEASE_MS,
        }
    } else if l.is_sonorant() && is_obstruent(r) {
        match closure_from_offglide(segs, i) {
            Some(ms) => ms,
            None if lc == Class::Nasal => CONSONANT_CONSONANT_MS,
            None if rc == Class::Fricative => VOWEL_FRICATIVE_MS,
            None => STOP_CLOSURE_MS,
        }
    } else if lc == Class::Vowel && rc == Class::Vowel {
        VOWEL_VOWEL_MS
    } else if lc == Class::Vowel || rc == Class::Vowel {
        match if lc == Class::Vowel { rc } else { lc } {
            Class::Liquid => VOWEL_LIQUID_MS,
            Class::Glide => GLIDE_VOWEL_MS,
            _ => NASAL_MS,
        }
    } else if (lc == Class::Nasal) != (rc == Class::Nasal) && l.is_sonorant() && r.is_sonorant() {
        NASAL_MS
    } else {
        CONSONANT_CONSONANT_MS
    };
    if segs[i].flap || segs[i + 1].flap {
        ms.min(FLAP_MS)
    } else {
        ms
    }
}

/// Aspiration is superimposed on the release transition, which it spans
/// exactly (Hertz 1991, pp. 96-97; Hertz & Huffman 1992, p. 1172).
fn aspirated(segs: &[Seg], i: usize) -> bool {
    segs[i + 1].aspiration_ms > 0.0
}

/// Share of the transition between `i` and `i + 1` that lies inside segment
/// `i`'s span.
fn share_before(segs: &[Seg], i: usize) -> f64 {
    let (Some(l), Some(r)) = (segs[i].ph, segs[i + 1].ph) else {
        return 0.0;
    };
    match (l.is_vowel(), r.is_vowel()) {
        // /h/ aspirates the transition between the phones around it (Hertz
        // 1991, p. 97), so the transition is shared with it.
        _ if l.class() == Class::Aspirate || r.class() == Class::Aspirate => 0.5,
        // Conventional segmentation splits a vowel-vowel transition halfway
        // (Hertz 1991, p. 97).
        (true, true) => 0.5,
        // Voiced transitions into and out of the vowel belong to its nucleus
        // and take their time from the vowel (Hertz & Huffman 1992, p. 1173).
        (true, false) => 1.0,
        (false, true) => 0.0,
        _ if l.is_sonorant() && is_obstruent(r) => 1.0,
        _ if is_obstruent(l) && r.is_sonorant() => 0.0,
        _ => 0.5,
    }
}

/// Transition duration (ms) between `i` and `i + 1` at a duration `scale`.
fn transition_ms(segs: &[Seg], i: usize, scale: f64) -> f64 {
    if aspirated(segs, i) {
        segs[i + 1].aspiration_ms
    } else {
        published_ms(segs, i) * TRANSITION_SCALE * scale.min(1.0).powf(FAST_RATE_EXPONENT)
    }
}

/// The parts of the transitions on each side that lie inside each segment, at
/// a duration `scale` (1 for the natural rate), before any steady-state floor.
fn raw_parts(segs: &[Seg], scale: f64) -> Vec<(f64, f64)> {
    let mut parts = vec![(0.0, 0.0); segs.len()];
    for i in 0..segs.len().saturating_sub(1) {
        let t = transition_ms(segs, i, scale);
        let share = share_before(segs, i);
        parts[i].1 = t * share;
        parts[i + 1].0 = t * (1.0 - share);
    }
    parts
}

/// Times the transitions for segments whose spans (`dur_ms`) are set, at a
/// duration `scale`. Every phone keeps a steady state of at least one frame;
/// where the transitions would leave less, they are shortened in proportion
/// and still reach their targets.
pub fn assign(segs: &mut [Seg], scale: f64) {
    let parts = raw_parts(segs, scale);
    for (s, (into, out)) in segs.iter_mut().zip(parts) {
        let available = (s.dur_ms - FRAME_MS).max(0.0);
        let k = if into + out > available {
            available / (into + out)
        } else {
            1.0
        };
        s.trans_in_ms = into * k;
        s.trans_out_ms = out * k;
    }
}

/// Natural-rate transition time inside the span of the coda liquid or glide
/// at `coda`. The nucleus moves it out of the vowel's span into the coda's,
/// because the non-vowel phones of a nucleus keep their assigned durations
/// and the vowel takes whatever is left (Hertz & Huffman 1992, p. 1173).
pub fn coda_transition_ms(segs: &[Seg], coda: usize) -> f64 {
    let parts = raw_parts(&segs[coda - 1..(coda + 2).min(segs.len())], 1.0);
    parts[1].0 + parts[1].1
}

/// The phone's steady state: its span less the transitions inside it.
pub fn steady_ms(seg: &Seg) -> f64 {
    seg.dur_ms - seg.trans_in_ms - seg.trans_out_ms
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::prosody::{assign_durations, scale_durations};
    use crate::text::normalize;
    use crate::utterance::build;

    fn timed(text: &str, scale: f64) -> Vec<Seg> {
        let mut segs = build(&normalize(text));
        assign_durations(&mut segs);
        scale_durations(&mut segs, scale);
        segs
    }

    fn find(segs: &[Seg], ph: Ph) -> usize {
        segs.iter().position(|s| s.ph == Some(ph)).unwrap()
    }

    #[test]
    fn lengthening_stretches_the_steady_state_not_the_transitions() {
        let (bead, beat) = (timed("bead", 1.0), timed("beat", 1.0));
        let (v, w) = (find(&bead, Ph::IY), find(&beat, Ph::IY));
        assert_eq!(bead[v].trans_in_ms, beat[w].trans_in_ms);
        assert_eq!(bead[v].trans_out_ms, beat[w].trans_out_ms);
        assert!(steady_ms(&bead[v]) > steady_ms(&beat[w]) + 20.0);
    }

    #[test]
    fn transition_durations_depend_on_the_phones_they_join() {
        let release = |text: &str, vowel: Ph| {
            let segs = timed(text, 1.0);
            segs[find(&segs, vowel)].trans_in_ms
        };
        let stop = release("a bee", Ph::IY);
        assert_eq!(stop, STOP_RELEASE_MS * TRANSITION_SCALE);
        assert_eq!(
            release("a sea", Ph::IY),
            FRICATIVE_RELEASE_MS * TRANSITION_SCALE
        );
        let tea = timed("tea", 1.0);
        let v = find(&tea, Ph::IY);
        assert_eq!(tea[v].trans_in_ms, tea[v].aspiration_ms);
        let ba = timed("bah", 1.0);
        let a = find(&ba, Ph::AA);
        assert_eq!(ba[a].trans_in_ms, stop);
    }

    #[test]
    fn fast_speech_shortens_transitions_more_than_phones_and_slow_speech_keeps_them() {
        let natural = timed("a bee", 1.0);
        let v = find(&natural, Ph::IY);
        for scale in [0.5, 1.0 / 3.0] {
            let fast = timed("a bee", scale);
            assert!((fast[v].dur_ms - natural[v].dur_ms * scale).abs() < 1e-9);
            let ratio = fast[v].trans_in_ms / natural[v].trans_in_ms;
            assert!((ratio - scale.powf(FAST_RATE_EXPONENT)).abs() < 1e-9);
            assert!(steady_ms(&fast[v]) > steady_ms(&natural[v]) * scale);
        }
        let slow = timed("a bee", 2.0);
        assert_eq!(slow[v].trans_in_ms, natural[v].trans_in_ms);
        assert!(steady_ms(&slow[v]) > 2.0 * steady_ms(&natural[v]));
    }

    #[test]
    fn the_vowel_pays_for_transitions_inside_its_nucleus() {
        let segs = timed("held", 1.0);
        let (v, l) = (find(&segs, Ph::EH), find(&segs, Ph::L));
        assert_eq!(segs[v].trans_out_ms, VOWEL_LIQUID_MS * TRANSITION_SCALE);
        assert_eq!(segs[l].trans_in_ms, 0.0);
        assert_eq!(
            segs[l].trans_out_ms,
            CODA_SONORANT_CLOSURE_MS * TRANSITION_SCALE
        );
    }

    #[test]
    fn every_phone_keeps_a_steady_frame_at_extreme_rates() {
        let text = "The quick brown fox, a strange idea, jumped over the lazy dogs.";
        for scale in [1.0, 0.3, 0.1] {
            for s in timed(text, scale).iter().filter(|s| s.ph.is_some()) {
                assert!(s.trans_in_ms >= 0.0 && s.trans_out_ms >= 0.0);
                assert!(
                    steady_ms(s) >= FRAME_MS.min(s.dur_ms) - 1e-9,
                    "{:?} at {scale}: {} ms span, {} ms steady",
                    s.ph,
                    s.dur_ms,
                    steady_ms(s)
                );
            }
        }
    }
}
