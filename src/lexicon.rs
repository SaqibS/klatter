//! Dictionary for pronunciation overrides and ambiguous inflection roots.
//! Vowels carry stress digits: 1 primary, 2 secondary, 0 unstressed.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::phoneme::Ph;

/// Caller-owned overrides; dictionary stress is still subject to connected-speech
/// deaccenting, and /t/ and /d/ still flap, but vowels are not reduced.
#[derive(Clone, Debug, Default)]
pub struct Pronunciations {
    entries: HashMap<String, Vec<(Ph, u8)>>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PronunciationError(String);

impl std::fmt::Display for PronunciationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PronunciationError {}

impl Pronunciations {
    /// Validates one normalized English word and ARPAbet phones before replacing an entry.
    pub fn insert(&mut self, word: &str, pronunciation: &str) -> Result<(), PronunciationError> {
        if word.is_empty()
            || word.starts_with('\'')
            || word.ends_with('\'')
            || !word.bytes().all(|c| c.is_ascii_alphabetic() || c == b'\'')
        {
            return Err(PronunciationError("override needs one English word".into()));
        }
        let mut phones = Vec::new();
        for token in pronunciation.split_whitespace() {
            let (base, stress) = token
                .strip_suffix(['0', '1', '2'])
                .map_or((token, None), |base| {
                    (base, Some(token.as_bytes()[token.len() - 1] - b'0'))
                });
            let ph = Ph::parse(base)
                .ok_or_else(|| PronunciationError(format!("unknown phone: {token}")))?;
            if ph.is_vowel() != stress.is_some() {
                return Err(PronunciationError(format!(
                    "only vowels require stress 0, 1 or 2: {token}"
                )));
            }
            let stress = stress.unwrap_or(0);
            phones.push((
                if ph == Ph::AH && stress == 0 {
                    Ph::AX
                } else {
                    ph
                },
                stress,
            ));
        }
        if phones.is_empty() {
            return Err(PronunciationError("override pronunciation is empty".into()));
        }
        self.entries.insert(word.to_ascii_uppercase(), phones);
        Ok(())
    }

    pub(crate) fn get(&self, word: &str) -> Option<&[(Ph, u8)]> {
        self.entries.get(word).map(Vec::as_slice)
    }
}

// Some exact CMUdict matches are needed to disambiguate inflection roots, and
// a few entries correct CMUdict where it parts from American usage and from
// the word's own root ("fallen" and "falling" have the vowel of "fall").
const ENTRIES: &str = "
AN AX0 N
OF AX0 V
AND AE0 N D
WITH W IH0 TH
WAS W AX0 Z
BECAUSE B IH0 K AH1 Z
YES Y EH1 S
ALICE AE1 L IH0 S
FIFTEEN F IH1 F T IY1 N
SIXTEEN S IH1 K S T IY1 N
EIGHTEEN EY1 T IY1 N
THIRTY TH ER1 T IY0
HUNDRED HH AH1 N D R IX0 D
ZERO Z IY1 R OW0
DOLLARS D AA1 L ER0 Z
KYOO K Y UW1
AYCH EY1 CH
READ R IY1 D
SAYS S EH1 Z
LIVE L IH1 V
WOMEN W IH1 M IX0 N
BUS B AH1 S
GAS G AE1 S
HOUSES HH AW1 Z IH0 Z
USES Y UW1 Z IH0 Z
ABUSES AX0 B Y UW1 Z IH0 Z
TOMORROW T AX0 M AA1 R OW0
LANGUAGE L AE1 NG G W IH0 JH
SYNTHESIS S IH1 N TH AX0 S IH0 S
KLATTER K L AE1 T ER0
DENNIS D EH1 N IH0 S
BEAUTIFUL B Y UW1 T IH0 F AX0 L
MINUTE M IH1 N IH0 T
EVERY EH1 V R IY0
NOBODY N OW1 B AA2 D IY0
IMPORTANT IH0 M P AO1 R T AX0 N T
AREN'T AA1 R N T
WE'RE W IH0 R
FALLEN F AO1 L AX0 N
FALLING F AO1 L IH0 NG
";

/// Dictionary stress is independent of connected-speech deaccenting.
pub fn lookup(word: &str) -> Option<Vec<(String, u8)>> {
    let phones = lookup_phones(word)
        .map(|phones| phones.to_vec())
        .or_else(|| cmudict_phones(word))?;
    Some(
        phones
            .iter()
            .map(|&(ph, stress)| (ph.name().to_string(), stress))
            .collect(),
    )
}

pub(crate) fn lookup_phones(word: &str) -> Option<&'static [(Ph, u8)]> {
    static DICTIONARY: OnceLock<HashMap<&'static str, Vec<(Ph, u8)>>> = OnceLock::new();
    DICTIONARY
        .get_or_init(|| {
            let mut entries = HashMap::new();
            for line in ENTRIES.lines() {
                let mut parts = line.split_whitespace();
                let Some(word) = parts.next() else { continue };
                let phones = parts.map(|p| parse_phone(word, p)).collect();
                assert!(
                    entries.insert(word, phones).is_none(),
                    "duplicate dictionary entry: {word}"
                );
            }
            entries
        })
        .get(word)
        .map(Vec::as_slice)
}

fn parse_phone(word: &str, p: &str) -> (Ph, u8) {
    let (name, stress) = match p.strip_suffix(|c: char| c.is_ascii_digit()) {
        Some(stem) => (stem, p.as_bytes()[p.len() - 1] - b'0'),
        None => (p, 0),
    };
    let ph = Ph::parse(name).unwrap_or_else(|| panic!("bad phoneme {name} in {word}"));
    (ph, stress)
}

// Binary distributions must include data/cmudict/LICENSE.txt.
const CMUDICT: &str = include_str!(concat!(env!("OUT_DIR"), "/cmudict.txt"));

pub(crate) fn cmudict_phones(word: &str) -> Option<Vec<(Ph, u8)>> {
    static LINES: OnceLock<Vec<&'static str>> = OnceLock::new();
    let lines = LINES.get_or_init(|| CMUDICT.lines().collect());
    let index = lines
        .binary_search_by(|line| {
            line.split_once(' ')
                .expect("CMUdict entry needs phonemes")
                .0
                .cmp(word)
        })
        .ok()?;
    Some(
        lines[index]
            .split_whitespace()
            .skip(1)
            .map(|p| parse_phone(word, p))
            .collect(),
    )
}

pub(crate) fn known_phones(word: &str, overrides: &Pronunciations) -> Option<Vec<(Ph, u8)>> {
    overrides
        .get(word)
        .or_else(|| lookup_phones(word))
        .map(<[(Ph, u8)]>::to_vec)
        .or_else(|| cmudict_phones(word))
}

pub(crate) fn derive_known(word: &str, overrides: &Pronunciations) -> Option<Vec<(Ph, u8)>> {
    use Ph::*;
    if !word.bytes().all(|c| c.is_ascii_uppercase()) {
        return None;
    }
    if let Some(stem) = word.strip_suffix("ISE")
        && let Some(phones) = known_phones(&format!("{stem}IZE"), overrides)
    {
        return Some(phones);
    }
    const SUFFIXES: &[(&str, &[(Ph, u8)])] = &[
        ("NESS", &[(N, 0), (AX, 0), (S, 0)]),
        ("LESS", &[(L, 0), (AX, 0), (S, 0)]),
        ("FUL", &[(F, 0), (AX, 0), (L, 0)]),
        ("MENTS", &[(M, 0), (AX, 0), (N, 0), (T, 0), (S, 0)]),
        ("MENT", &[(M, 0), (AX, 0), (N, 0), (T, 0)]),
        ("ABLE", &[(AX, 0), (B, 0), (AX, 0), (L, 0)]),
        ("ISH", &[(IH, 0), (SH, 0)]),
        ("ISM", &[(IH, 0), (Z, 0), (AX, 0), (M, 0)]),
        ("EST", &[(AX, 0), (S, 0), (T, 0)]),
        ("ER", &[(ER, 0)]),
    ];
    for (suffix, ending) in SUFFIXES {
        // EER carries its own stress, unlike the agent/comparative ER suffix.
        if *suffix == "ER" && word.ends_with("EER") {
            continue;
        }
        if let Some(stem) = word.strip_suffix(suffix).filter(|s| s.len() >= 2)
            && let Some(mut phones) = known_stem(stem, overrides)
        {
            phones.extend_from_slice(ending);
            return Some(phones);
        }
    }
    // Product names and identifiers must not trigger a quadratic split search.
    if word.len() > 64 {
        return None;
    }
    let mut compound = None;
    for split in 4..word.len().saturating_sub(3) {
        let (left, right) = word.split_at(split);
        if crate::utterance::FUNCTION_WORDS.contains(&left) {
            continue;
        }
        if let (Some(mut first), Some(second)) = (
            known_phones(left, overrides),
            known_phones(right, overrides),
        ) && first
            .iter()
            .any(|&(ph, stress)| ph.is_vowel() && stress == 1)
            && second.iter().any(|&(ph, _)| ph.is_vowel())
        {
            // Ambiguous segmentations are weaker evidence than the whole-word rules.
            if compound.is_some() {
                return None;
            }
            first.extend(
                second
                    .into_iter()
                    .map(|(ph, stress)| (ph, if stress == 1 { 2 } else { stress })),
            );
            compound = Some(first);
        }
    }
    compound
}

fn known_stem(stem: &str, overrides: &Pronunciations) -> Option<Vec<(Ph, u8)>> {
    known_phones(stem, overrides)
        .or_else(|| known_phones(&format!("{stem}E"), overrides))
        .or_else(|| {
            let bytes = stem.as_bytes();
            (bytes.len() >= 2
                && bytes[bytes.len() - 1] == bytes[bytes.len() - 2]
                && !b"AEIOUY".contains(&bytes[bytes.len() - 1]))
            .then(|| known_phones(&stem[..stem.len() - 1], overrides))
            .flatten()
        })
        .or_else(|| {
            stem.strip_suffix('I')
                .and_then(|s| known_phones(&format!("{s}Y"), overrides))
        })
}

#[cfg(test)]
mod tests {
    use crate::phoneme::Ph;

    #[test]
    fn new_derivations_require_known_stems_and_unambiguous_compounds() {
        let mut overrides = super::Pronunciations::default();
        overrides.insert("glorp", "G L AO1 R P").unwrap();
        assert_eq!(
            super::derive_known("GLORPFUL", &overrides).unwrap(),
            vec![
                (Ph::G, 0),
                (Ph::L, 0),
                (Ph::AO, 1),
                (Ph::R, 0),
                (Ph::P, 0),
                (Ph::F, 0),
                (Ph::AX, 0),
                (Ph::L, 0)
            ]
        );
        assert!(super::derive_known("XYZZYFUL", &overrides).is_none());
        assert_eq!(
            super::derive_known("BOOKDOCTOR", &overrides).unwrap(),
            vec![
                (Ph::B, 0),
                (Ph::UH, 1),
                (Ph::K, 0),
                (Ph::D, 0),
                (Ph::AA, 2),
                (Ph::K, 0),
                (Ph::T, 0),
                (Ph::ER, 0)
            ]
        );
        for word in ["ZORA", "ZORAVAKA", "VAKATILO", "TILO"] {
            overrides.insert(word, "B AE1 T").unwrap();
        }
        assert!(super::derive_known("ZORAVAKATILO", &overrides).is_none());
        for word in ["COOKIN", "LOOKIN", "SHOOTIN", "STANDIN", "SOMETHIN"] {
            assert!(super::derive_known(word, &overrides).is_none(), "{word}");
        }
        assert!(super::derive_known("GAZETTEER", &overrides).is_none());
        assert_eq!(
            super::derive_known("THEORISE", &overrides).unwrap(),
            vec![
                (Ph::TH, 0),
                (Ph::IY, 1),
                (Ph::ER, 0),
                (Ph::AY, 2),
                (Ph::Z, 0)
            ]
        );
        overrides.insert("bookdoctor", "B UH1 K").unwrap();
        assert_eq!(
            crate::utterance::pronounce_with("BOOKDOCTOR", &overrides),
            vec![(Ph::B, 0), (Ph::UH, 1), (Ph::K, 0)]
        );
    }

    #[test]
    fn invalid_overrides_do_not_replace_valid_entries() {
        let mut dictionary = super::Pronunciations::default();
        dictionary.insert("read", "R EH1 D").unwrap();
        for (word, phones) in [
            ("", "R"),
            ("two words", "R"),
            ("'read", "R"),
            ("read", ""),
            ("read", "R AE"),
            ("read", "R1 EH1 D"),
            ("read", "R AE3 D"),
            ("read", "R UNKNOWN1 D"),
        ] {
            assert!(dictionary.insert(word, phones).is_err(), "{word}: {phones}");
        }
        assert_eq!(
            dictionary.get("READ"),
            Some([(Ph::R, 0), (Ph::EH, 1), (Ph::D, 0)].as_slice())
        );
    }

    #[test]
    fn every_entry_uses_known_phonemes() {
        for line in super::ENTRIES.lines().filter(|l| !l.trim().is_empty()) {
            let word = line.split_whitespace().next().unwrap();
            for (ph, _) in super::lookup(word).unwrap() {
                assert!(Ph::parse(&ph).is_some(), "{word}: {ph}");
            }
        }
    }

    #[test]
    fn finds_stress() {
        let hello = super::lookup("HELLO").unwrap();
        assert_eq!(hello[1], ("AX".to_string(), 0));
        assert_eq!(hello[3], ("OW".to_string(), 1));
        assert_eq!(super::lookup("READ").unwrap()[1], ("IY".to_string(), 1));
        assert!(super::lookup("TEH").is_none());
    }

    #[test]
    fn cmudict_is_sorted_unique_and_parses() {
        let mut previous = "";
        for line in super::CMUDICT.lines() {
            let (word, _) = line.split_once(' ').expect("word and phonemes");
            assert!(previous < word, "out-of-order or duplicate entry: {word}");
            let phones = super::cmudict_phones(word).expect("dictionary entry");
            assert!(!phones.is_empty(), "{word}");
            assert!(
                phones
                    .iter()
                    .all(|(ph, stress)| *stress <= 2 && (ph.is_vowel() || *stress == 0)),
                "{word}"
            );
            previous = word;
        }
    }
}
