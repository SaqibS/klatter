//! Morphological decomposition before letter-to-sound conversion, after the
//! suffix-stripping procedure of Hertz (1986), "English text to speech rules
//! with Delta". Stripping inflectional suffixes lets dictionary entries and
//! letter-to-sound rules apply to the root, and lets the suffix's sound follow
//! the root's final phoneme.

use crate::{lexicon, phoneme::Ph};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Suffix {
    S,
    Ed,
    Ing,
    Ly,
}

/// Words that look inflected but are not, or whose root would be
/// pronounced wrongly on its own.
const NO_SPLIT: &[&str] = &[
    "HUNDRED",
    "KINDRED",
    "HATRED",
    "SACRED",
    "NAKED",
    "RUGGED",
    "RAGGED",
    "JAGGED",
    "WICKED",
    "WRETCHED",
    "CROOKED",
    "BELOVED",
    "EMBED",
    "DURING",
    "EVENING",
    "FAMILY",
    "EARLY",
    "HOLY",
    "UGLY",
    "BUTTERFLY",
    "SERIES",
    "SPECIES",
    "CHAOS",
];

fn is_vowel_letter(word: &[u8], i: usize) -> bool {
    match word[i] {
        b'A' | b'E' | b'I' | b'O' => true,
        // The U of QU is part of the consonant.
        b'U' => i == 0 || word[i - 1] != b'Q',
        b'Y' => i > 0,
        _ => false,
    }
}

fn vowel_groups(word: &[u8]) -> usize {
    (0..word.len())
        .filter(|&i| is_vowel_letter(word, i) && (i == 0 || !is_vowel_letter(word, i - 1)))
        .count()
}

/// A one-syllable root spelled vowel + single consonant at the end lost a
/// silent E before the suffix (hoping, liked), whereas a doubled consonant
/// (hopping) or a vowel digraph (rained) means there was none.
fn one_syllable_cvc(stem: &[u8]) -> bool {
    let n = stem.len();
    n >= 2 && vowel_groups(stem) == 1 && ends_in_single_vowel_consonant(stem)
}

fn ends_in_single_vowel_consonant(stem: &[u8]) -> bool {
    let n = stem.len();
    n >= 2
        && !is_vowel_letter(stem, n - 1)
        && !matches!(stem[n - 1], b'W' | b'X' | b'Y')
        && is_vowel_letter(stem, n - 2)
        && (n == 2 || !is_vowel_letter(stem, n - 3))
}

/// English roots do not end in V, C, U or a G after a consonant (except NG),
/// nor in a vowel digraph + S or Z, so the suffix must have replaced an E
/// (proved, danced, argued, judged, paused). After NG, only A and U take it
/// (changed, plunged; but belonged, ringed).
fn must_have_lost_e(stem: &[u8]) -> bool {
    let n = stem.len();
    let at = |k: usize| if k <= n && k > 0 { stem[n - k] } else { b' ' };
    match at(1) {
        b'V' | b'U' => true,
        b'C' => true,
        b'G' => match at(2) {
            b'N' => matches!(at(3), b'A' | b'U'),
            b'R' | b'D' | b'L' => true,
            _ => false,
        },
        b'S' | b'Z' => n >= 3 && is_vowel_letter(stem, n - 2) && is_vowel_letter(stem, n - 3),
        _ => false,
    }
}

/// The root before -ED or -ING, or `None` when spelling cannot tell whether
/// an E was dropped (visited vs. invited); those words stay whole.
fn root_before_vowel_suffix(stem: &str) -> Option<String> {
    let s = stem.as_bytes();
    if must_have_lost_e(s) || one_syllable_cvc(s) {
        Some(format!("{stem}E"))
    } else if vowel_groups(s) > 1 && ends_in_single_vowel_consonant(s) {
        // Unstressed -EN, -ER, -EL never had an E (opened, covered, travelled).
        let n = s.len();
        (s[n - 2] == b'E' && matches!(s[n - 1], b'N' | b'R' | b'L')).then(|| stem.to_string())
    } else {
        Some(stem.to_string())
    }
}

fn valid_root(root: &str) -> bool {
    root.len() >= 2 && vowel_groups(root.as_bytes()) > 0
}

/// Splits an uppercase word into a root and an inflectional suffix, repairing
/// the root's spelling (tried -> TRY, hoping -> HOPE, busily -> BUSY).
pub fn split(word: &str) -> Option<(String, Suffix)> {
    if NO_SPLIT.contains(&word) || !word.bytes().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    let (root, suffix) = if let Some(stem) = word.strip_suffix("ING") {
        (root_before_vowel_suffix(stem)?, Suffix::Ing)
    } else if let Some(stem) = word.strip_suffix("ED") {
        // need, freed: the E belongs to the root.
        if stem.ends_with('E') {
            return None;
        }
        let root = match stem.strip_suffix('I') {
            Some(s) => format!("{s}Y"),
            None => root_before_vowel_suffix(stem)?,
        };
        (root, Suffix::Ed)
    } else if let Some(stem) = word.strip_suffix("LY") {
        // simply, humbly, deeply come from roots in -le or are safer whole;
        // holy, truly end the root in a vowel that the suffix would mask.
        let last = *stem.as_bytes().last()?;
        if matches!(last, b'B' | b'P' | b'A' | b'O' | b'U') {
            return None;
        }
        let root = match stem.strip_suffix('I') {
            Some(s) => format!("{s}Y"),
            None => stem.to_string(),
        };
        (root, Suffix::Ly)
    } else if let Some(stem) = word.strip_suffix("IES") {
        (format!("{stem}Y"), Suffix::S)
    } else if ["SSES", "XES", "ZZES", "CHES", "SHES"]
        .iter()
        .any(|e| word.ends_with(e))
    {
        (word[..word.len() - 2].to_string(), Suffix::S)
    } else if word.ends_with("SES") || word.ends_with("ZES") {
        // Spelling alone cannot distinguish BUS + ES from LOSE + S.
        let s_root = &word[..word.len() - 1];
        let es_root = &word[..word.len() - 2];
        // Function words are not evidence for a lexical root, as in USES versus US.
        let known_root = |root: &str| {
            lexicon::lookup(root).is_some_and(|entry| entry.iter().any(|(_, stress)| *stress > 0))
        };
        let root = match (known_root(s_root), known_root(es_root)) {
            // Root-level LTS keeps the suffix from changing lexical stress.
            (_, false) => s_root,
            (false, true) => es_root,
            (true, true) => return None,
        };
        (root.to_string(), Suffix::S)
    } else {
        let stem = word.strip_suffix('S')?;
        if ["S", "U", "I", "A"].iter().any(|e| stem.ends_with(e)) {
            return None;
        }
        (stem.to_string(), Suffix::S)
    };
    valid_root(&root).then_some((root, suffix))
}

/// The suffix's phonemes, which depend on the root's final phoneme.
pub fn suffix_phones(root_last: Option<Ph>, suffix: Suffix) -> Vec<(Ph, u8)> {
    use Ph::*;
    match suffix {
        Suffix::S => match root_last {
            Some(S | Z | SH | ZH | CH | JH) => vec![(IH, 0), (Z, 0)],
            Some(P | T | K | F | TH) => vec![(S, 0)],
            _ => vec![(Z, 0)],
        },
        Suffix::Ed => match root_last {
            Some(T | D) => vec![(IH, 0), (D, 0)],
            Some(P | K | F | TH | S | SH | CH) => vec![(T, 0)],
            _ => vec![(D, 0)],
        },
        Suffix::Ing => vec![(IH, 0), (NG, 0)],
        Suffix::Ly if root_last == Some(L) => vec![(IY, 0)],
        Suffix::Ly => vec![(L, 0), (IY, 0)],
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn root(word: &str) -> Option<String> {
        split(word).map(|(r, _)| r)
    }

    #[test]
    fn repairs_root_spelling() {
        assert_eq!(root("BUSILY").as_deref(), Some("BUSY"));
        assert_eq!(root("TRIED").as_deref(), Some("TRY"));
        assert_eq!(root("CITIES").as_deref(), Some("CITY"));
        assert_eq!(root("HOPING").as_deref(), Some("HOPE"));
        assert_eq!(root("HOPPED").as_deref(), Some("HOPP"));
        assert_eq!(root("RAINED").as_deref(), Some("RAIN"));
        assert_eq!(root("QUOTED").as_deref(), Some("QUOTE"));
        assert_eq!(root("BOXES").as_deref(), Some("BOX"));
        assert_eq!(root("PLACES").as_deref(), Some("PLACE"));
        assert_eq!(root("CHANGED").as_deref(), Some("CHANGE"));
        assert_eq!(root("BELONGED").as_deref(), Some("BELONG"));
        assert_eq!(root("JUDGED").as_deref(), Some("JUDGE"));
        assert_eq!(root("DANCED").as_deref(), Some("DANCE"));
        assert_eq!(root("PAUSED").as_deref(), Some("PAUSE"));
        assert_eq!(root("PROVED").as_deref(), Some("PROVE"));
        assert_eq!(root("OPENED").as_deref(), Some("OPEN"));
        assert_eq!(root("VISITED"), None);
    }

    #[test]
    fn leaves_uninflected_words_whole() {
        for w in [
            "HUNDRED", "NAKED", "THING", "STRING", "BED", "NEED", "SIMPLY", "HOLY", "THUS",
            "CLASS", "GAS",
        ] {
            assert_eq!(split(w), None, "{w}");
        }
    }

    #[test]
    fn ambiguous_plurals_use_known_roots() {
        assert_eq!(root("BUSES").as_deref(), Some("BUS"));
        assert_eq!(root("GASES").as_deref(), Some("GAS"));
        assert_eq!(root("LOSES").as_deref(), Some("LOSE"));
        assert_eq!(root("ROSES").as_deref(), Some("ROSE"));
        assert_eq!(root("NOSES").as_deref(), Some("NOSE"));
        assert_eq!(root("USES").as_deref(), Some("USE"));
        assert_eq!(root("AMAZES").as_deref(), Some("AMAZE"));
        assert_eq!(root("AMUSES").as_deref(), Some("AMUSE"));
        assert_eq!(root("ABUSES").as_deref(), Some("ABUSE"));
    }

    #[test]
    fn suffix_follows_root_voicing() {
        let names = |v: Vec<(Ph, u8)>| v.iter().map(|p| p.0.name()).collect::<Vec<_>>().join(" ");
        assert_eq!(names(suffix_phones(Some(Ph::T), Suffix::Ed)), "IH D");
        assert_eq!(names(suffix_phones(Some(Ph::P), Suffix::Ed)), "T");
        assert_eq!(names(suffix_phones(Some(Ph::N), Suffix::Ed)), "D");
        assert_eq!(names(suffix_phones(Some(Ph::CH), Suffix::S)), "IH Z");
        assert_eq!(names(suffix_phones(Some(Ph::K), Suffix::S)), "S");
        assert_eq!(names(suffix_phones(Some(Ph::L), Suffix::Ly)), "IY");
    }
}
