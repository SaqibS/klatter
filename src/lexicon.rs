//! Dictionary for function words, irregular spellings and ambiguous inflection roots.
//! Vowels carry stress digits: 1 primary, 2 secondary, 0 unstressed.
//! Words whose vowels are all unstressed are treated as function words.

use std::collections::HashMap;
use std::sync::OnceLock;

use crate::phoneme::Ph;

const ENTRIES: &str = "
A AX0
AN AX0 N
THE DH AX0
OF AX0 V
TO T UW0
AND AE0 N D
IN IH0 N
IS IH0 Z
IT IH0 T
ITS IH0 T S
THAT DH AE0 T
FOR F AO0 R
ON AA0 N
WITH W IH0 TH
AS AE0 Z
AT AE0 T
BY B AY0
FROM F R AH0 M
OR AO0 R
NOR N AO0 R
BUT B AH0 T
BE B IY0
ARE AA0 R
WAS W AH0 Z
WERE W ER0
AM AE0 M
BEEN B IH0 N
HE HH IY0
SHE SH IY0
WE W IY0
YOU Y UW0
THEY DH EY0
I AY0
ME M IY0
HIM HH IH0 M
HER HH ER0
US AH0 S
THEM DH EH0 M
MY M AY0
YOUR Y AO0 R
HIS HH IH0 Z
OUR AW0 ER0
THEIR DH EH0 R
THIS DH IH0 S
THESE DH IY0 Z
THOSE DH OW0 Z
HAVE HH AE0 V
HAS HH AE0 Z
HAD HH AE0 D
DO D UW0
DOES D AH0 Z
DID D IH0 D
WILL W IH0 L
WOULD W UH0 D
CAN K AE0 N
COULD K UH0 D
SHOULD SH UH0 D
SHALL SH AE0 L
MAY M EY0
MUST M AH0 S T
SO S OW0
IF IH0 F
THAN DH AE0 N
THEN DH EH0 N
THERE DH EH0 R
WHEN W EH0 N
WHAT W AH0 T
WHICH W IH0 CH
WHO HH UW0
WHOM HH UW0 M
WHOSE HH UW0 Z
WHERE W EH0 R
HOW HH AW0
WHY W AY0
EACH IY0 CH
INTO IH0 N T UW0
ONTO AA0 N T UW0
UPON AX0 P AA1 N
ABOUT AX0 B AW1 T
BEFORE B IH0 F AO1 R
BECAUSE B IH0 K AH1 Z
THROUGH TH R UW0
UP AH0 P
DOWN D AW1 N
NOT N AA1 T
NO N OW1
YES Y EH1 S
ALL AO1 L
SOME S AH0 M
ANY EH1 N IY0
MANY M EH1 N IY0
ONLY OW1 N L IY0
ALSO AO1 L S OW0
VERY V EH1 R IY0
OTHER AH1 DH ER0
ANOTHER AX0 N AH1 DH ER0
OVER OW1 V ER0
UNDER AH1 N D ER0
AFTER AE1 F T ER0
AGAIN AX0 G EH1 N
AGAINST AX0 G EH1 N S T
ALICE AE1 L IH0 S
CONSIDER K AX0 N S IH1 D ER0
ONE W AH1 N
ONCE W AH1 N S
TWO T UW1
THREE TH R IY1
FOUR F AO1 R
FIVE F AY1 V
SIX S IH1 K S
SEVEN S EH1 V AX0 N
EIGHT EY1 T
NINE N AY1 N
TEN T EH1 N
ELEVEN IH0 L EH1 V AX0 N
TWELVE T W EH1 L V
THIRTEEN TH ER1 T IY1 N
FOURTEEN F AO1 R T IY1 N
FIFTEEN F IH1 F T IY1 N
SIXTEEN S IH1 K S T IY1 N
SEVENTEEN S EH1 V AX0 N T IY1 N
EIGHTEEN EY1 T IY1 N
NINETEEN N AY1 N T IY1 N
TWENTY T W EH1 N T IY0
THIRTY TH ER1 T IY0
FORTY F AO1 R T IY0
FIFTY F IH1 F T IY0
SIXTY S IH1 K S T IY0
SEVENTY S EH1 V AX0 N T IY0
EIGHTY EY1 T IY0
NINETY N AY1 N T IY0
HUNDRED HH AH1 N D R IX0 D
THOUSAND TH AW1 Z AX0 N D
MILLION M IH1 L Y AX0 N
BILLION B IH1 L Y AX0 N
TRILLION T R IH1 L Y AX0 N
ZERO Z IY1 R OW0
POINT P OY1 N T
PERCENT P ER0 S EH1 N T
DOLLAR D AA1 L ER0
DOLLARS D AA1 L ER0 Z
FIRST F ER1 S T
SECOND S EH1 K AX0 N D
THIRD TH ER1 D
EIGHTH EY1 T TH
OH OW1
DOUBLE D AH1 B AX0 L
KYOO K Y UW1
AYCH EY1 CH
HELLO HH AX0 L OW1
WORLD W ER1 L D
SAID S EH1 D
READ R IY1 D
SAYS S EH1 Z
GIVE G IH1 V
LIVE L IH1 V
LOVE L AH1 V
COME K AH1 M
DONE D AH1 N
GONE G AO1 N
PEOPLE P IY1 P AX0 L
WATER W AO1 T ER0
FRIEND F R EH1 N D
WOMAN W UH1 M AX0 N
WOMEN W IH1 M IX0 N
EYE AY1
BUSY B IH1 Z IY0
BUS B AH1 S
GAS G AE1 S
CITY S IH1 T IY0
HOUSES HH AW1 Z IH0 Z
HANGED HH AE1 NG D
BUY B AY1
LAUGH L AE1 F
ENOUGH IH0 N AH1 F
THOUGH DH OW1
THOUGHT TH AO1 T
TOUGH T AH1 F
OCEAN OW1 SH AX0 N
SURE SH UH1 R
SUGAR SH UH1 G ER0
ISLAND AY1 L AX0 N D
ANSWER AE1 N S ER0
OFTEN AO1 F AX0 N
LISTEN L IH1 S AX0 N
KNOW N OW1
KNEW N UW1
DOOR D AO1 R
FLOOR F L AO1 R
MOVE M UW1 V
PROVE P R UW1 V
LOSE L UW1 Z
TODAY T AX0 D EY1
TOMORROW T AX0 M AA1 R OW0
YESTERDAY Y EH1 S T ER0 D EY2
ENGLISH IH1 NG G L IH0 SH
LANGUAGE L AE1 NG G W IH0 JH
SPEECH S P IY1 CH
SYNTHESIS S IH1 N TH AX0 S IH0 S
SYNTHESIZER S IH1 N TH AX0 S AY2 Z ER0
COMPUTER K AX0 M P Y UW1 T ER0
VOICE V OY1 S
TEXT T EH1 K S T
KLATT K L AE1 T
KLATTER K L AE1 T ER0
DENNIS D EH1 N IH0 S
HAVING HH AE1 V IH0 NG
GOOD G UH1 D
GREAT G R EY1 T
BREAK B R EY1 K
STEAK S T EY1 K
HEART HH AA1 R T
BEAUTIFUL B Y UW1 T IH0 F UH0 L
QUESTION K W EH1 S CH AX0 N
MINUTE M IH1 N IH0 T
EVERY EH1 V R IY0
EVERYONE EH1 V R IY0 W AH2 N
EVERYTHING EH1 V R IY0 TH IH2 NG
SOMETHING S AH1 M TH IH0 NG
NOTHING N AH1 TH IH0 NG
ANYTHING EH1 N IY0 TH IH2 NG
SOMEONE S AH1 M W AH2 N
ANYONE EH1 N IY0 W AH2 N
NOBODY N OW1 B AA2 D IY0
SOMEBODY S AH1 M B AA2 D IY0
BODY B AA1 D IY0
EXAMPLE IH0 G Z AE1 M P AX0 L
CHILDREN CH IH1 L D R AX0 N
MACHINE M AX0 SH IY1 N
SHOE SH UW1
BUSINESS B IH1 Z N AX0 S
POLICE P AX0 L IY1 S
NATURE N EY1 CH ER0
CREATE K R IY0 EY1 T
IMPORTANT IH0 M P AO1 R T AX0 N T
UNDERSTAND AH2 N D ER0 S T AE1 N D
QUICK K W IH1 K
BROWN B R AW1 N
LAZY L EY1 Z IY0
DOG D AO1 G
WOULDN'T W UH1 D AX0 N T
COULDN'T K UH1 D AX0 N T
SHOULDN'T SH UH1 D AX0 N T
DON'T D OW1 N T
WON'T W OW1 N T
CAN'T K AE1 N T
ISN'T IH1 Z AX0 N T
AREN'T AA1 R N T
WASN'T W AA1 Z AX0 N T
DOESN'T D AH1 Z AX0 N T
DIDN'T D IH1 D AX0 N T
I'M AY0 M
I'LL AY0 L
I'VE AY0 V
I'D AY0 D
IT'S IH0 T S
THAT'S DH AE0 T S
LET'S L EH1 T S
YOU'RE Y UH0 R
THEY'RE DH EH0 R
WE'RE W IH0 R
";

pub fn lookup(word: &str) -> Option<Vec<(String, u8)>> {
    lookup_phones(word).map(|phones| {
        phones
            .iter()
            .map(|&(ph, stress)| (ph.name().to_string(), stress))
            .collect()
    })
}

pub(crate) fn lookup_phones(word: &str) -> Option<&'static [(Ph, u8)]> {
    static DICTIONARY: OnceLock<HashMap<&'static str, Vec<(Ph, u8)>>> = OnceLock::new();
    DICTIONARY
        .get_or_init(|| {
            let mut entries = HashMap::new();
            for line in ENTRIES.lines() {
                let mut parts = line.split_whitespace();
                let Some(word) = parts.next() else { continue };
                let phones = parts
                    .map(|p| {
                        let (name, stress) = match p.strip_suffix(|c: char| c.is_ascii_digit()) {
                            Some(stem) => (stem, p.as_bytes()[p.len() - 1] - b'0'),
                            None => (p, 0),
                        };
                        let ph = Ph::parse(name)
                            .unwrap_or_else(|| panic!("bad phoneme {name} in {word}"));
                        (ph, stress)
                    })
                    .collect();
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

#[cfg(test)]
mod tests {
    use crate::phoneme::Ph;

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
    }
}
