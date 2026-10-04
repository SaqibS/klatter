//! Letter-to-sound rules in the style of the NRL report (Elovitz et al., 1976),
//! as used by Klatt-era systems, plus heuristic lexical stress assignment.
//!
//! Each rule is `left|MATCH|right|PHONEMES`. The first rule (in order) whose
//! match and contexts fit wins. Context symbols:
//! - ` ` word boundary
//! - `#` one or more vowels
//! - `:` zero or more consonants
//! - `^` one consonant
//! - `.` one voiced consonant
//! - `+` front vowel (E, I, Y)
//! - `&` sibilant (S, C, G, Z, X, J, CH, SH)
//! - `@` consonant that makes a following U long (T S R D L Z N J TH CH SH)
//! - `%` suffix (ER, E, ES, ED, ING, ELY), right context only

use crate::phoneme::Ph;
use std::sync::OnceLock;

const RULES: &str = r"
 |A| |AX
 |ARE| |AA R
 |AR|O|AX R
|AR|#|EH R
^|AS|#|EY S
|A|WA|AX
|AW||AO
 :|ANY||EH N IY
|ATION||EY SH AX N
|A|^+#|EY
#:|ALLY||AX L IY
 |AL|#|AX L
|AGAIN||AX G EH N
#:|AG|E|IH JH
|A|^%|EY
|A|^+:#|AE
 :|A|^+ |EY
 |ARR||AX R
|ARR||AE R
 :|AR| |AA R
|AR| |ER
|AR||AA R
|AIR||EH R
|AI||EY
|AY||EY
|AU||AO
#:|AL| |AX L
#:|ALS| |AX L Z
|ALK||AO K
|AL|^|AO L
 :|ABLE||EY B AX L
|ABLE||AX B AX L
|ANG|+|EY N JH
|A||AE

 |BE|^#|B IH
|BEING||B IY IH NG
 |BOTH| |B OW TH
|BUIL||B IH L
|BB||B
M|B| |
|B||B

 |CH|^|K
^E|CH||K
|CH||CH
 S|CI|#|S AY
|CI|A|SH
|CI|O|SH
|CI|EN|SH
|CK||K
|COM|%|K AH M
|CC|+|K S
|C|+|S
|CC||K
|C||K

#:|DED| |D IH D
.E|D| |D
#^:E|D| |T
 |DE|^#|D IH
|DU|A|JH UW
|DG|E|JH
|DD||D
|D||D

#:|E| |
 :|E| |IY
#|ED| |D
#:|E|D |
|EV|ER|EH V
|E|^%|IY
|ERI|#|IY R IY
|ERI||EH R IH
#:|ER|#|ER
|ER|#|EH R
|ER||ER
 |EVEN||IY V EH N
#:|E|W|
@|EW||UW
|EW||Y UW
|E|O|IY
#:&|ES| |IH Z
#:|E|S |
#:|ELY| |L IY
#:|EMENT||M EH N T
|EFUL||F UH L
|EE||IY
|EARN||ER N
 |EAR|^|ER
|EAD||EH D
#:|EA| |IY AX
|EA|SU|EH
|EA||IY
|EIGH||EY
|EI||IY
 |EYE||AY
|EY||IY
|EU||Y UW
|E||EH

|FUL||F UH L
|FF||F
|F||F

|GIV||G IH V
 |G|I^|G
|GE|T|G EH
SU|GGES||G JH EH S
|GG||G
 B#|G||G
|G|+|JH
|GREAT||G R EY T
#|GH||
|GN| |N
|G||G

 |HAV||HH AE V
 |HERE||HH IY R
 |HOUR||AW ER
|HOW||HH AW
|H|#|HH
|H||

 |IN||IH N
 |I| |AY
|IN|D|AY N
|IER||IY ER
 :|IED| |AY D
|IED| |IY D
 :|IES| |AY Z
|IES| |IY Z
|IEN||IY EH N
|IE|T|AY EH
 :|IE| |AY
|IE|^|IY
 :|I|%|AY
|I|%|IY
|IE||IY
|I|^+:#|IH
|IR|#|AY R
|IZ|%|AY Z
|IS|%|AY Z
|I|D%|AY
+^|I|^+|IH
|I|T%|AY
#^:|I|^+|IH
|I|^+|AY
|IR||ER
|IGH||AY
|ILD||AY L D
|IGN| |AY N
|IGN|^|AY N
|IGN|%|AY N
|IQUE||IY K
|I||IH

|J||JH

 |K|N|
|K||K

|LO|C#|L OW
L|L||
#^:|L|%|AX L
|LEAD||L IY D
|L||L

|MOV||M UW V
|MM||M
|M||M

E|NG|+|N JH
|NG|R|NG G
|NG|#|NG G
|NGL|%|NG G AX L
|NG||NG
|NK||NG K
 |NOW| |N AW
|NN||N
|N||N

|OF| |AX V
|OROUGH||ER OW
#:|OR| |ER
#:|ORS| |ER Z
|OR||AO R
 |ONE||W AH N
|OW||OW
 |OVER||OW V ER
|OV||AH V
|O|^%|OW
|O|^EN|OW
|O|^I#|OW
|OL|D|OW L
|OUGHT||AO T
|OUGH||AH F
 |OU||AW
|OUSE| |AW S
H|OU|S#|AW
|OUS||AX S
|OUR||AO R
|OULD||UH D
^|OU|^L|AH
|OUP||UW P
|OU||AW
|OY||OY
|OING||OW IH NG
|OI||OY
|OOR||AO R
|OOK||UH K
|OOD||UH D
|OO||UW
|O|E|OW
|O| |OW
|OA||OW
 |ONLY||OW N L IY
 |ONCE||W AH N S
|ON'T||OW N T
C|O|N|AA
|O|NG|AO
 :^|O|N|AH
I|ON||AX N
#:|ON| |AX N
#^|ON||AX N
|O|ST |OW
|OF|^|AO F
|OTHER||AH DH ER
|OSS| |AO S
#^:|OM||AH M
|O||AA

|PH||F
|PEOP||P IY P
|POW||P AW
|PUT| |P UH T
|PP||P
|P||P

|QUAR||K W AO R
|QU||K W
|Q||K

 |RE|^#|R IY
|RR||R
|R||R

|SH||SH
#|SION||ZH AX N
|SOME||S AH M
#|SUR|#|ZH ER
|SUR|#|SH ER
#|SU|#|ZH UW
#|SSU|#|SH UW
#|SED| |Z D
#|S|#|Z
|SAID||S EH D
^|SION||SH AX N
|S|S|
.|S| |Z
#:.E|S| |Z
#^:##|S| |Z
#^:#|S| |S
U|S| |S
 :#|S| |Z
 |SCH||S K
|S|C+|
#|SM||Z M
|S||S

 |THE| |DH AX
|TO| |T UW
|THAT| |DH AE T
 |THIS| |DH IH S
 |THEY||DH EY
 |THERE||DH EH R
|THER||DH ER
|THEIR||DH EH R
 |THAN| |DH AE N
 |THEM| |DH EH M
|THESE| |DH IY Z
 |THEN||DH EH N
|THROUGH||TH R UW
|THOSE||DH OW Z
|THOUGH| |DH OW
 |THUS||DH AH S
|TH||TH
#:|TED| |T IH D
S|TI|#N|CH
|TI|O|SH
|TI|A|SH
|TIEN||SH AX N
|TUR|#|CH ER
|TU|A|CH UW
 |TWO||T UW
|TCH||CH
|TT||T
|T||T

 |UN|I|Y UW N
 |UN||AH N
 |UPON||AX P AO N
@|UR|#|UH R
|UR|#|Y UH R
|UR||ER
|U|^ |AH
|U|^^|AH
|UY||AY
 G|U|#|
G|U|%|
G|U|#|W
#N|U||Y UW
@|U||UW
|U||Y UW

|VIEW||V Y UW
|V||V

 |WERE||W ER
|WA|S|W AA
|WA|T|W AA
|WHERE||W EH R
|WHAT||W AH T
|WHOL||HH OW L
|WHO||HH UW
|WH||W
|WAR||W AO R
|WOR|^|W ER
|WR||R
|W||W

|X||K S

|YOUNG||Y AH NG
 |YOU||Y UW
 |YES||Y EH S
 |Y||Y
#^:|Y| |IY
#^:|Y|I|IY
 :|Y| |AY
 :|Y|#|AY
 :|Y|^+:#|IH
 :|Y|^#|AY
|Y||IH

|ZZ||Z
|Z||Z

|'S||Z
|'||
";

struct Rule {
    left: Vec<u8>,
    matched: Vec<u8>,
    right: Vec<u8>,
    out: Vec<Ph>,
}

fn rules() -> &'static [Rule] {
    static RULES_CELL: OnceLock<Vec<Rule>> = OnceLock::new();
    RULES_CELL.get_or_init(|| {
        RULES
            .lines()
            .filter(|l| l.contains('|'))
            .map(|line| {
                let parts: Vec<&str> = line.split('|').collect();
                assert_eq!(parts.len(), 4, "malformed rule: {line:?}");
                Rule {
                    left: parts[0].bytes().rev().collect(),
                    matched: parts[1].as_bytes().to_vec(),
                    right: parts[2].as_bytes().to_vec(),
                    out: parts[3]
                        .split_whitespace()
                        .map(|p| {
                            Ph::parse(p).unwrap_or_else(|| panic!("bad phoneme {p} in {line:?}"))
                        })
                        .collect(),
                }
            })
            .collect()
    })
}

fn is_vowel(c: u8) -> bool {
    b"AEIOUY".contains(&c)
}

fn is_consonant(c: u8) -> bool {
    c.is_ascii_uppercase() && !is_vowel(c)
}

fn is_voiced_consonant(c: u8) -> bool {
    b"BDVGJLMNRWZ".contains(&c)
}

fn is_front_vowel(c: u8) -> bool {
    b"EIY".contains(&c)
}

fn is_boundary(c: u8) -> bool {
    !c.is_ascii_uppercase() && c != b'\''
}

/// Matches a context pattern starting at `pos`, stepping by `dir` (+1 for
/// right contexts, -1 for left contexts, whose patterns are stored reversed).
/// Variable-length symbols backtrack so `:^` can still find its consonant.
fn match_context(word: &[u8], pos: isize, pattern: &[u8], dir: isize) -> bool {
    let Some((&sym, rest)) = pattern.split_first() else {
        return true;
    };
    let at = |p: isize| {
        if p < 0 {
            b' '
        } else {
            word.get(p as usize).copied().unwrap_or(b' ')
        }
    };
    let next = |p: isize| match_context(word, p, rest, dir);
    let c = at(pos);
    match sym {
        b' ' => is_boundary(c) && next(pos + dir),
        b'#' => {
            let mut p = pos;
            while is_vowel(at(p)) {
                p += dir;
                if next(p) {
                    return true;
                }
            }
            false
        }
        b':' => {
            let mut p = pos;
            loop {
                if next(p) {
                    return true;
                }
                if !is_consonant(at(p)) {
                    return false;
                }
                p += dir;
            }
        }
        b'^' => is_consonant(c) && next(pos + dir),
        b'.' => is_voiced_consonant(c) && next(pos + dir),
        b'+' => is_front_vowel(c) && next(pos + dir),
        b'&' | b'@' => {
            let pair = if dir > 0 {
                [c, at(pos + 1)]
            } else {
                [at(pos - 1), c]
            };
            let digraph = pair == *b"CH" || pair == *b"SH" || (sym == b'@' && pair == *b"TH");
            let single: &[u8] = if sym == b'&' { b"SCGZXJ" } else { b"TSRDLZNJ" };
            (digraph && next(pos + 2 * dir)) || (single.contains(&c) && next(pos + dir))
        }
        b'%' => [&b"ING"[..], b"ELY", b"ER", b"ES", b"ED", b"E"]
            .iter()
            .any(|s| {
                let end = pos + s.len() as isize;
                // A suffix must end the word, optionally pluralized (MAKERS).
                let ends_word =
                    is_boundary(at(end)) || (at(end) == b'S' && is_boundary(at(end + 1)));
                s.iter()
                    .enumerate()
                    .all(|(k, &ch)| at(pos + k as isize) == ch)
                    && ends_word
                    && next(end)
            }),
        lit => c == lit && next(pos + dir),
    }
}

/// Converts an uppercase word to phonemes using the letter-to-sound rules.
pub fn letters_to_phonemes(word: &str) -> Vec<Ph> {
    let w = word.as_bytes();
    let mut out = Vec::new();
    let mut pos = 0;
    while pos < w.len() {
        let rule = rules().iter().find(|r| {
            w[pos..].starts_with(&r.matched)
                && match_context(w, pos as isize - 1, &r.left, -1)
                && match_context(w, (pos + r.matched.len()) as isize, &r.right, 1)
        });
        match rule {
            Some(r) => {
                out.extend_from_slice(&r.out);
                pos += r.matched.len();
            }
            None => pos += 1,
        }
    }
    out
}

const UNSTRESSED_PREFIXES: &[&str] = &["BE", "DE", "RE", "PRE", "EX"];

/// Leading "a" before a single consonant and a vowel in a two-syllable word
/// is usually an unstressed prefix (along, away, alone).
fn has_schwa_prefix(word: &str) -> bool {
    let w = word.as_bytes();
    w.len() >= 4 && w[0] == b'A' && is_consonant(w[1]) && is_vowel(w[2])
}

/// Suffixes that attract stress to the syllable before them, with the number
/// of syllables the suffix itself contributes.
const PRE_STRESSING_SUFFIXES: &[(&str, usize)] = &[
    ("ATION", 1),
    ("TION", 1),
    ("SION", 1),
    ("CIAN", 1),
    ("TIAL", 1),
    ("CIAL", 1),
    ("ICAL", 2),
    ("ICALLY", 3),
    ("ITY", 2),
    ("IOUS", 1),
    ("EOUS", 1),
    ("IC", 1),
    ("ICS", 1),
    ("UAL", 2),
    ("IAN", 2),
    ("IA", 2),
    ("OLOGY", 2),
    ("OGRAPHY", 2),
];

/// Assigns primary stress to one vowel and reduces unstressed AH to schwa.
pub fn assign_stress(word: &str, phones: &[Ph]) -> Vec<(Ph, u8)> {
    let vowels: Vec<usize> = (0..phones.len())
        .filter(|&i| phones[i].is_vowel())
        .collect();
    let n = vowels.len();
    let stressed_syllable = if n <= 1 {
        0
    } else if let Some(&(_, k)) = PRE_STRESSING_SUFFIXES
        .iter()
        .find(|(s, _)| word.ends_with(s) && word.len() > s.len() + 1)
    {
        n.saturating_sub(k + 1)
    } else if (n == 2 && has_schwa_prefix(word))
        || ((2..=3).contains(&n)
            && UNSTRESSED_PREFIXES
                .iter()
                .any(|p| word.starts_with(p) && word.len() >= p.len() + 3))
    {
        1
    } else {
        0
    };
    let stressed_index = vowels.get(stressed_syllable).copied();
    let schwa_prefix = has_schwa_prefix(word) && stressed_syllable == 1;
    phones
        .iter()
        .enumerate()
        .map(|(i, &p)| {
            if Some(i) == stressed_index {
                (p, 1)
            } else if (p == Ph::AH && n > 1) || (schwa_prefix && i == 0 && p == Ph::AE) {
                (Ph::AX, 0)
            } else {
                (p, 0)
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pron(word: &str) -> String {
        letters_to_phonemes(word)
            .iter()
            .map(|p| p.name())
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn rules_parse() {
        assert!(rules().len() > 250);
    }

    #[test]
    fn common_words() {
        assert_eq!(pron("CAT"), "K AE T");
        assert_eq!(pron("MAKE"), "M EY K");
        assert_eq!(pron("SHIP"), "SH IH P");
        assert_eq!(pron("THINK"), "TH IH NG K");
        assert_eq!(pron("NATION"), "N EY SH AX N");
        assert_eq!(pron("PHONE"), "F OW N");
        assert_eq!(pron("JUMPS"), "JH AH M P S");
        assert_eq!(pron("FOX"), "F AA K S");
        assert_eq!(pron("HAPPY"), "HH AE P IY");
        assert_eq!(pron("DOGS"), "D AA G Z");
        assert_eq!(pron("SIT"), "S IH T");
        assert_eq!(pron("KNIGHT"), "N AY T");
    }

    #[test]
    fn stress_heuristics() {
        let s = |w: &str| {
            assign_stress(w, &letters_to_phonemes(w))
                .iter()
                .filter(|(p, _)| p.is_vowel())
                .map(|(_, s)| *s)
                .collect::<Vec<_>>()
        };
        assert_eq!(s("CAT"), vec![1]);
        assert_eq!(s("NATION"), vec![1, 0]);
        assert_eq!(s("BEGIN"), vec![0, 1]);
        assert_eq!(s("ALONG"), vec![0, 1]);
        assert_eq!(s("ANIMAL"), vec![1, 0, 0]);
        assert_eq!(s("TECHNOLOGY"), vec![0, 1, 0, 0]);
    }
}
