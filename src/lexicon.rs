//! Dictionary for pronunciation overrides and ambiguous inflection roots.
//! Vowels carry stress digits: 1 primary, 2 secondary, 0 unstressed.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::phoneme::Ph;

/// Caller-owned overrides; dictionary stress is still subject to connected-speech deaccenting.
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

// Some exact CMUdict matches are needed to disambiguate inflection roots.
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

#[cfg(test)]
mod tests {
    use crate::phoneme::Ph;

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
