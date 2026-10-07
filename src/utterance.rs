//! Builds the segment sequence that the prosody and parameter stages work on:
//! words become phonemes annotated with syllable, stress and phrase position.

use crate::lexicon;
use crate::lts;
use crate::morph::{self, Suffix};
use crate::phoneme::Ph;
use crate::text::{Break, Token};

#[derive(Clone, Debug)]
pub struct Seg {
    /// `None` is a pause.
    pub ph: Option<Ph>,
    /// For pauses: the break that caused it (`None` for the utterance edges).
    pub brk: Option<Break>,
    /// Stress of the syllable this segment belongs to (0, 1 or 2).
    pub stress: u8,
    pub function_word: bool,
    pub word_initial: bool,
    pub word_final: bool,
    pub syllable: usize,
    pub syllables_in_word: usize,
    /// True for segments after the nucleus of their syllable.
    pub coda: bool,
    /// Syllable is the last one before a pause.
    pub phrase_final: bool,
    pub dur_ms: f64,
    /// Unvoiced onset budget within this segment, scaled with its duration.
    pub aspiration_ms: f64,
}

impl Seg {
    fn pause(brk: Option<Break>) -> Seg {
        Seg {
            ph: None,
            brk,
            stress: 0,
            function_word: false,
            word_initial: true,
            word_final: true,
            syllable: 0,
            syllables_in_word: 0,
            coda: false,
            phrase_final: false,
            dur_ms: 0.0,
            aspiration_ms: 0.0,
        }
    }

    pub fn is_vowel(&self) -> bool {
        self.ph.is_some_and(Ph::is_vowel)
    }
}

pub fn pronounce(word: &str) -> Vec<(Ph, u8)> {
    if let Some(entry) = lexicon::lookup_phones(word) {
        return entry.to_vec();
    }
    let split = match word.strip_suffix("'S").filter(|b| !b.is_empty()) {
        Some(base) => Some((base.to_string(), Suffix::S)),
        None => morph::split(word),
    };
    if let Some((root, suffix)) = split {
        let mut phones = pronounce(&root);
        if !phones.is_empty() {
            // Roots like DO and BE are unstressed function words, but doing and being are not.
            if suffix != Suffix::S
                && phones.iter().all(|p| p.1 == 0)
                && let Some(v) = phones.iter_mut().find(|p| p.0.is_vowel())
            {
                v.1 = 1;
            }
            phones.extend(morph::suffix_phones(phones.last().map(|p| p.0), suffix));
            return phones;
        }
    }
    let letters: String = word.chars().filter(|c| c.is_ascii_alphabetic()).collect();
    let phones = lts::letters_to_phonemes(word);
    lts::assign_stress(&letters, &phones)
}

fn legal_onset(cluster: &[Ph]) -> bool {
    use Ph::*;
    match cluster {
        [] => true,
        [NG] | [ZH] => false,
        [_] => true,
        [P | B | K | G | F | TH | SH, R]
        | [T | D, R]
        | [P | B | K | G | F | S, L]
        | [S, P | T | K | M | N | W]
        | [T | D | K | G, W]
        | [P | B | K | F | M | V | HH, Y] => true,
        [S, P | T | K, R] | [S, P | K, L] | [S, K, W] | [S, P | K, Y] => true,
        _ => false,
    }
}

/// Returns, for each phone, its syllable index and whether it is a coda.
fn syllabify(phones: &[(Ph, u8)]) -> Vec<(usize, bool)> {
    let nuclei: Vec<usize> = (0..phones.len())
        .filter(|&i| phones[i].0.is_vowel())
        .collect();
    let mut result = vec![(0, false); phones.len()];
    if nuclei.is_empty() {
        return result;
    }
    for (s, &v) in nuclei.iter().enumerate() {
        result[v] = (s, false);
    }
    for i in 0..nuclei[0] {
        result[i] = (0, false);
    }
    let last = *nuclei.last().unwrap();
    for i in last + 1..phones.len() {
        result[i] = (nuclei.len() - 1, true);
    }
    for (s, pair) in nuclei.windows(2).enumerate() {
        let (a, b) = (pair[0], pair[1]);
        let cluster: Vec<Ph> = phones[a + 1..b].iter().map(|p| p.0).collect();
        let onset_start = (0..=cluster.len())
            .find(|&k| legal_onset(&cluster[k..]))
            .unwrap_or(cluster.len());
        for (k, i) in (a + 1..b).enumerate() {
            result[i] = if k < onset_start {
                (s, true)
            } else {
                (s + 1, false)
            };
        }
    }
    result
}

pub fn build(tokens: &[Token]) -> Vec<Seg> {
    build_with_marks(tokens).0
}

/// Also returns each mark with the index of the segment that follows it; marks
/// before any speech precede the leading pause.
pub fn build_with_marks(tokens: &[Token]) -> (Vec<Seg>, Vec<(u32, usize)>) {
    let mut segs = vec![Seg::pause(None)];
    let mut marks = Vec::new();
    for token in tokens {
        match token {
            Token::Mark(id) => {
                let speech_started = segs.iter().any(|s| s.ph.is_some());
                marks.push((*id, if speech_started { segs.len() } else { 0 }));
            }
            Token::Break(b) => {
                if segs.last().is_some_and(|s| s.ph.is_some()) {
                    segs.push(Seg::pause(Some(*b)));
                }
            }
            Token::Word(w) => {
                let phones = pronounce(w);
                if phones.is_empty() {
                    continue;
                }
                let syl = syllabify(&phones);
                let n_syl = phones.iter().filter(|p| p.0.is_vowel()).count().max(1);
                let function_word = phones.iter().all(|p| p.1 == 0);
                let mut stress_of = vec![0u8; n_syl];
                for (i, p) in phones.iter().enumerate() {
                    if p.0.is_vowel() {
                        stress_of[syl[i].0] = p.1;
                    }
                }
                for (i, &(ph, _)) in phones.iter().enumerate() {
                    segs.push(Seg {
                        ph: Some(ph),
                        brk: None,
                        stress: stress_of[syl[i].0],
                        function_word,
                        word_initial: i == 0,
                        word_final: i + 1 == phones.len(),
                        syllable: syl[i].0,
                        syllables_in_word: n_syl,
                        coda: syl[i].1,
                        phrase_final: false,
                        dur_ms: 0.0,
                        aspiration_ms: 0.0,
                    });
                }
            }
        }
    }
    if segs.last().is_some_and(|s| s.ph.is_some()) {
        segs.push(Seg::pause(Some(Break::Period)));
    }
    mark_phrase_final(&mut segs);
    (segs, marks)
}

fn mark_phrase_final(segs: &mut [Seg]) {
    let mut i = segs.len();
    while i > 0 {
        i -= 1;
        if segs[i].ph.is_some() {
            continue;
        }
        // Walk back to the start of the last word before this pause, then
        // mark the segments belonging to that word's final syllable.
        let mut j = i;
        while j > 0 && segs[j - 1].ph.is_some() {
            j -= 1;
            if segs[j].word_initial {
                break;
            }
        }
        let last_syllable = segs[j..i].iter().map(|s| s.syllable).max();
        if let Some(ls) = last_syllable {
            for s in &mut segs[j..i] {
                if s.syllable == ls {
                    s.phrase_final = true;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::text::normalize;

    #[test]
    fn possessive_follows_voicing_of_base() {
        let names = |w: &str| pronounce(w).iter().map(|p| p.0).collect::<Vec<_>>();
        assert_eq!(names("KLATT'S"), vec![Ph::K, Ph::L, Ph::AE, Ph::T, Ph::S]);
        assert_eq!(*names("DOG'S").last().unwrap(), Ph::Z);
        assert_eq!(names("JUDGE'S")[3..], [Ph::IH, Ph::Z]);
    }

    #[test]
    fn dictionary_stress_survives_inflection() {
        use Ph::*;
        assert_eq!(pronounce("ALICE"), vec![(AE, 1), (L, 0), (IH, 0), (S, 0)]);
        let root = vec![(K, 0), (AX, 0), (N, 0), (S, 0), (IH, 1), (D, 0), (ER, 0)];
        assert_eq!(pronounce("CONSIDER"), root);
        for (word, suffix) in [
            ("CONSIDERS", vec![(Z, 0)]),
            ("CONSIDERED", vec![(D, 0)]),
            ("CONSIDERING", vec![(IH, 0), (NG, 0)]),
        ] {
            let mut expected = root.clone();
            expected.extend(suffix);
            assert_eq!(pronounce(word), expected, "{word}");
        }
    }

    #[test]
    fn down_and_inflections_keep_the_aw_vowel() {
        use Ph::*;
        let root = vec![(D, 0), (AW, 1), (N, 0)];
        assert_eq!(pronounce("DOWN"), root);
        for (word, suffix) in [
            ("DOWNS", vec![(Z, 0)]),
            ("DOWNED", vec![(D, 0)]),
            ("DOWNING", vec![(IH, 0), (NG, 0)]),
        ] {
            let mut expected = root.clone();
            expected.extend(suffix);
            assert_eq!(pronounce(word), expected, "{word}");
        }
        let segs = build(&normalize("down"));
        let phones: Vec<_> = segs.iter().filter_map(|s| s.ph).collect();
        assert_eq!(phones, vec![D, AW, N]);
        let vowel = segs.iter().find(|s| s.is_vowel()).unwrap();
        assert_eq!(vowel.stress, 1);
        assert!(!vowel.function_word);
    }

    #[test]
    fn ambiguous_plural_pronunciations() {
        use Ph::*;
        for (word, expected) in [
            ("BUSES", vec![B, AH, S, IH, Z]),
            ("GASES", vec![G, AE, S, IH, Z]),
            ("LOSES", vec![L, UW, Z, IH, Z]),
            ("USES", vec![Y, UW, Z, IH, Z]),
            ("ROSES", vec![R, OW, Z, IH, Z]),
            ("BOXES", vec![B, AA, K, S, IH, Z]),
            ("PLACES", vec![P, L, EY, S, IH, Z]),
            ("HOUSES", vec![HH, AW, Z, IH, Z]),
        ] {
            let actual: Vec<Ph> = pronounce(word).into_iter().map(|p| p.0).collect();
            assert_eq!(actual, expected, "{word}");
        }
    }

    #[test]
    fn inflected_words_preserve_root_stress_and_vowels() {
        use Ph::*;
        for (word, expected) in [
            (
                "AMAZES",
                vec![(AX, 0), (M, 0), (EY, 1), (Z, 0), (IH, 0), (Z, 0)],
            ),
            (
                "AMUSES",
                vec![(AX, 0), (M, 0), (Y, 0), (UW, 1), (Z, 0), (IH, 0), (Z, 0)],
            ),
            (
                "ABUSES",
                vec![(AX, 0), (B, 0), (Y, 0), (UW, 1), (Z, 0), (IH, 0), (Z, 0)],
            ),
        ] {
            assert_eq!(pronounce(word), expected, "{word}");
        }
        for root in [
            "AMAZE",
            "AMUSE",
            "ABUSE",
            "ARISE",
            "EXERCISE",
            "RECOGNIZE",
            "MOUSE",
        ] {
            let mut expected = pronounce(root);
            expected.extend([(IH, 0), (Z, 0)]);
            assert_eq!(pronounce(&format!("{root}S")), expected, "{root} + S");
        }
    }

    #[test]
    fn syllabifies_with_maximal_onset() {
        let phones: Vec<(Ph, u8)> = [Ph::EH, Ph::K, Ph::S, Ph::T, Ph::R, Ph::AX]
            .iter()
            .map(|&p| (p, 0))
            .collect();
        let s = syllabify(&phones);
        assert_eq!(
            s,
            vec![
                (0, false),
                (0, true),
                (1, false),
                (1, false),
                (1, false),
                (1, false)
            ]
        );
    }

    #[test]
    fn marks_phrase_final_syllable() {
        let segs = build(&normalize("hello world"));
        let finals: Vec<Ph> = segs
            .iter()
            .filter(|s| s.phrase_final)
            .filter_map(|s| s.ph)
            .collect();
        assert_eq!(finals, vec![Ph::W, Ph::ER, Ph::L, Ph::D]);
        assert!(segs.first().unwrap().ph.is_none());
        assert!(segs.last().unwrap().ph.is_none());
    }
}
