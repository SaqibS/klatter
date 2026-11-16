use std::collections::BTreeMap;

pub fn convert(source: &str) -> Result<String, String> {
    let mut entries = BTreeMap::new();
    for (index, line) in source.lines().enumerate() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.is_empty() || line.starts_with(";;;") {
            continue;
        }
        let mut parts = line.split_whitespace();
        let word = parts.next().expect("nonempty line").to_ascii_uppercase();
        if word.contains('(') || !word.bytes().all(|c| c.is_ascii_alphabetic() || c == b'\'') {
            continue;
        }
        let mut phones = Vec::new();
        for token in parts {
            let (base, stress) = token
                .strip_suffix(['0', '1', '2'])
                .map_or((token, false), |base| (base, true));
            let vowel = matches!(
                base,
                "AA" | "AE"
                    | "AH"
                    | "AO"
                    | "AW"
                    | "AY"
                    | "EH"
                    | "ER"
                    | "EY"
                    | "IH"
                    | "IY"
                    | "OW"
                    | "OY"
                    | "UH"
                    | "UW"
            );
            let consonant = matches!(
                base,
                "B" | "CH"
                    | "D"
                    | "DH"
                    | "F"
                    | "G"
                    | "HH"
                    | "JH"
                    | "K"
                    | "L"
                    | "M"
                    | "N"
                    | "NG"
                    | "P"
                    | "R"
                    | "S"
                    | "SH"
                    | "T"
                    | "TH"
                    | "V"
                    | "W"
                    | "Y"
                    | "Z"
                    | "ZH"
            );
            if !(vowel && stress || consonant && !stress) {
                return Err(format!(
                    "line {} ({word}): invalid phone {token}",
                    index + 1
                ));
            }
            phones.push(if token == "AH0" { "AX0" } else { token });
        }
        if phones.is_empty() {
            return Err(format!(
                "line {} ({word}): missing pronunciation",
                index + 1
            ));
        }
        // CMUdict gives the reduced [di] first for some words spelled -day,
        // such as "Tuesday", "Monday" and "Saturday"; American dictionaries
        // such as Merriam-Webster give [deɪ] first, as CMUdict itself does for
        // "Sunday", "holiday" and "yesterday".
        if word.ends_with("DAY") && phones.ends_with(&["D", "IY0"]) {
            phones.pop();
            phones.push("EY2");
        }
        entries.entry(word).or_insert_with(|| phones.join(" "));
    }
    let mut out = String::new();
    for (word, phones) in entries {
        out.push_str(&word);
        out.push(' ');
        out.push_str(&phones);
        out.push('\n');
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    #[test]
    fn normalizes_validates_and_selects_first_reading() {
        let table = super::convert(
            ";;; header\nrabbit R AE1 B AH0 T # comment\n\
            read R EH1 D\nread(2) R IY1 D\nread R IY1 D\nalpha AE1 L F AH0\n",
        )
        .unwrap();
        assert_eq!(
            table,
            "ALPHA AE1 L F AX0\nRABBIT R AE1 B AX0 T\nREAD R EH1 D\n"
        );
        for invalid in ["x B1", "x AE", "x AH21", "x AX0", "x UNKNOWN1", "empty"] {
            assert!(super::convert(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn words_spelled_day_end_in_a_full_day() {
        let table = super::convert(
            "saturday S AE1 T ER0 D IY0\nsaturday(2) S AE1 T IH2 D EY2\ncandy K AE1 N D IY0\n",
        )
        .unwrap();
        assert_eq!(table, "CANDY K AE1 N D IY0\nSATURDAY S AE1 T ER0 D EY2\n");
    }
}
