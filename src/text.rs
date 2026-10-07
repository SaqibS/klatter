//! Text normalization: turns raw text into uppercase words and prosodic breaks.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Break {
    Comma,
    Period,
    Question,
    Exclamation,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Token {
    Word(String),
    Break(Break),
    /// A caller's position marker; it passes through to the audio untouched.
    Mark(u32),
}

const ONES: [&str; 20] = [
    "zero",
    "one",
    "two",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
];
const TENS: [&str; 10] = [
    "", "", "twenty", "thirty", "forty", "fifty", "sixty", "seventy", "eighty", "ninety",
];
const SCALES: [&str; 5] = ["", "thousand", "million", "billion", "trillion"];

const ABBREVIATIONS: &[(&str, &str)] = &[
    ("mr", "mister"),
    ("mrs", "missus"),
    ("ms", "miz"),
    ("dr", "doctor"),
    ("st", "saint"),
    ("jr", "junior"),
    ("sr", "senior"),
    ("prof", "professor"),
    ("vs", "versus"),
    ("etc", "et cetera"),
    ("approx", "approximately"),
];

fn below_thousand(n: u64, out: &mut Vec<String>) {
    let (h, rest) = (n / 100, n % 100);
    if h > 0 {
        out.push(ONES[h as usize].into());
        out.push("hundred".into());
    }
    if rest >= 20 {
        out.push(TENS[(rest / 10) as usize].into());
        if rest % 10 > 0 {
            out.push(ONES[(rest % 10) as usize].into());
        }
    } else if rest > 0 {
        out.push(ONES[rest as usize].into());
    }
}

pub fn number_words(n: u64) -> Vec<String> {
    if n == 0 {
        return vec!["zero".into()];
    }
    let mut groups = Vec::new();
    let mut m = n;
    while m > 0 {
        groups.push(m % 1000);
        m /= 1000;
    }
    if groups.len() > SCALES.len() {
        return digit_words(&n.to_string());
    }
    let mut out = Vec::new();
    for (i, &g) in groups.iter().enumerate().rev() {
        if g > 0 {
            below_thousand(g, &mut out);
            if i > 0 {
                out.push(SCALES[i].into());
            }
        }
    }
    out
}

fn digit_words(digits: &str) -> Vec<String> {
    digits
        .chars()
        .filter_map(|c| c.to_digit(10))
        .map(|d| ONES[d as usize].to_string())
        .collect()
}

/// Reads plausible years as pairs ("nineteen eighty four").
fn year_words(n: u64) -> Option<Vec<String>> {
    if !(1100..=2099).contains(&n) || (2000..2010).contains(&n) {
        return None;
    }
    let (hi, lo) = (n / 100, n % 100);
    let mut out = number_words(hi);
    match lo {
        0 => out.push("hundred".into()),
        1..=9 => {
            out.push("oh".into());
            out.extend(number_words(lo));
        }
        _ => out.extend(number_words(lo)),
    }
    Some(out)
}

fn ordinal(mut words: Vec<String>) -> Vec<String> {
    if let Some(last) = words.pop() {
        let ord = match last.as_str() {
            "one" => "first".to_string(),
            "two" => "second".to_string(),
            "three" => "third".to_string(),
            "five" => "fifth".to_string(),
            "eight" => "eighth".to_string(),
            "nine" => "ninth".to_string(),
            "twelve" => "twelfth".to_string(),
            w if w.ends_with('y') => format!("{}ieth", &w[..w.len() - 1]),
            w => format!("{w}th"),
        };
        words.push(ord);
    }
    words
}

fn letter_name(c: char) -> &'static str {
    match c.to_ascii_lowercase() {
        'a' => "ay",
        'b' => "bee",
        'c' => "see",
        'd' => "dee",
        'e' => "ee",
        'f' => "eff",
        'g' => "jee",
        'h' => "aych",
        'i' => "eye",
        'j' => "jay",
        'k' => "kay",
        'l' => "ell",
        'm' => "em",
        'n' => "en",
        'o' => "oh",
        'p' => "pee",
        'q' => "kyoo",
        'r' => "ar",
        's' => "ess",
        't' => "tee",
        'u' => "you",
        'v' => "vee",
        'w' => "double you",
        'x' => "ex",
        'y' => "why",
        'z' => "zee",
        _ => "",
    }
}

fn push_words(out: &mut Vec<Token>, words: impl IntoIterator<Item = String>) {
    for w in words {
        for part in w.split_whitespace() {
            out.push(Token::Word(part.to_ascii_uppercase()));
        }
    }
}

fn push_break(out: &mut Vec<Token>, b: Break) {
    match out.iter_mut().rev().find(|t| !matches!(t, Token::Mark(_))) {
        None => {}
        Some(Token::Break(prev)) => {
            // A stronger break absorbs a weaker one (e.g. "word," then ".").
            if *prev == Break::Comma {
                *prev = b;
            }
        }
        Some(_) => out.push(Token::Break(b)),
    }
}

fn is_word_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '\''
}

fn expand_number(num: &str, suffix: &str, allow_year: bool, out: &mut Vec<Token>) {
    let suffix_lower = suffix.to_ascii_lowercase();
    let value: Option<u64> = num.parse().ok();
    match (value, suffix_lower.as_str()) {
        (Some(n), "st" | "nd" | "rd" | "th") => push_words(out, ordinal(number_words(n))),
        (Some(n), "s") if n % 10 == 0 => {
            let mut w =
                if allow_year { year_words(n) } else { None }.unwrap_or_else(|| number_words(n));
            let last = w.pop().unwrap_or_default();
            let plural = match last.strip_suffix('y') {
                Some(stem) => format!("{stem}ies"),
                None => format!("{last}s"),
            };
            w.push(plural);
            push_words(out, w);
        }
        (Some(n), _) => {
            let words = if num.len() > 1 && num.starts_with('0') {
                digit_words(num)
            } else if allow_year && num.len() == 4 {
                year_words(n).unwrap_or_else(|| number_words(n))
            } else {
                number_words(n)
            };
            push_words(out, words);
            if !suffix.is_empty() {
                expand_word(suffix, out);
            }
        }
        (None, _) => {
            push_words(out, digit_words(num));
            if !suffix.is_empty() {
                expand_word(suffix, out);
            }
        }
    }
}

fn expand_word(raw: &str, out: &mut Vec<Token>) {
    let word = raw.trim_matches('\'');
    if word.is_empty() {
        return;
    }
    let lower = word.to_ascii_lowercase();

    let digits_end = word
        .find(|c: char| !c.is_ascii_digit())
        .unwrap_or(word.len());
    if digits_end > 0 {
        let (num, suffix) = word.split_at(digits_end);
        expand_number(num, suffix, true, out);
        return;
    }

    if let Some(&(_, full)) = ABBREVIATIONS.iter().find(|(a, _)| *a == lower) {
        push_words(out, [full.to_string()]);
        return;
    }

    let has_vowel = lower.chars().any(|c| "aeiouy".contains(c));
    let letters_only = word.chars().all(|c| c.is_ascii_alphabetic());
    let all_caps = word.len() > 1 && word.chars().all(|c| c.is_ascii_uppercase());
    if letters_only
        && (!has_vowel || (all_caps && word.len() <= 3 && !is_pronounceable_acronym(&lower)))
    {
        push_words(out, word.chars().map(|c| letter_name(c).to_string()));
        return;
    }
    if word.chars().any(|c| c.is_ascii_digit()) {
        // Mixed tokens like "mp3": split into letter and digit runs.
        let mut run = String::new();
        let mut run_is_digit = false;
        for c in word.chars() {
            if !run.is_empty() && c.is_ascii_digit() != run_is_digit {
                expand_word(&run, out);
                run.clear();
            }
            run_is_digit = c.is_ascii_digit();
            run.push(c);
        }
        expand_word(&run, out);
        return;
    }
    out.push(Token::Word(word.to_ascii_uppercase()));
}

fn is_pronounceable_acronym(lower: &str) -> bool {
    // Short all-caps words that are ordinary words in shouted text.
    matches!(
        lower,
        "a" | "i"
            | "ok"
            | "the"
            | "and"
            | "you"
            | "yes"
            | "no"
            | "not"
            | "but"
            | "for"
            | "are"
            | "was"
            | "is"
            | "it"
    )
}

fn is_thousands_separator(chars: &[char], i: usize) -> bool {
    let digit_at = |k: usize| chars.get(k).is_some_and(|c| c.is_ascii_digit());
    chars[i] == ','
        && i > 0
        && digit_at(i - 1)
        && digit_at(i + 1)
        && digit_at(i + 2)
        && digit_at(i + 3)
        && !digit_at(i + 4)
}

pub fn normalize(text: &str) -> Vec<Token> {
    let mut out = normalize_fragment(text);
    end_utterance(&mut out);
    out
}

/// Normalizes text that may continue in a later fragment, so it gets no
/// utterance-final break.
pub fn normalize_fragment(text: &str) -> Vec<Token> {
    let mut out = Vec::new();
    append_normalized(text, &mut out);
    out
}

/// Appends a word-boundary fragment using the preceding text's punctuation context.
pub fn append_normalized(text: &str, out: &mut Vec<Token>) {
    let chars: Vec<char> = text
        .chars()
        .map(|c| match c {
            '\u{2018}' | '\u{2019}' | '\u{02bc}' | '\u{ff07}' => '\'',
            _ => c,
        })
        .collect();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        let sign = c == '\u{2212}'
            || (c == '-' && (i == 0 || (!is_word_char(chars[i - 1]) && chars[i - 1] != '-')));
        if sign
            && chars.get(i + 1) == Some(&'$')
            && chars.get(i + 2).is_some_and(|c| c.is_ascii_digit())
        {
            push_words(out, ["minus".to_string()]);
            i += 1;
            continue;
        }
        let negative = sign && chars.get(i + 1).is_some_and(|c| c.is_ascii_digit());
        if c.is_ascii_digit() || negative {
            if negative {
                push_words(out, ["minus".to_string()]);
                i += 1;
            }
            // Gather numbers with thousands separators and decimals.
            let start = i;
            let mut j = i;
            while j < chars.len()
                && (chars[j].is_ascii_digit() || is_thousands_separator(&chars, j))
            {
                j += 1;
            }
            let int_part: String = chars[start..j].iter().filter(|c| **c != ',').collect();
            let allow_year = !negative && !chars[start..j].contains(&',');
            let mut words = Vec::new();
            if j + 1 < chars.len() && chars[j] == '.' && chars[j + 1].is_ascii_digit() {
                let mut k = j + 1;
                while k < chars.len() && chars[k].is_ascii_digit() {
                    k += 1;
                }
                let frac: String = chars[j + 1..k].iter().collect();
                words.extend(
                    int_part
                        .parse::<u64>()
                        .map(number_words)
                        .unwrap_or_else(|_| digit_words(&int_part)),
                );
                words.push("point".into());
                words.extend(digit_words(&frac));
                push_words(out, words);
                i = k;
                continue;
            }
            if j < chars.len() && chars[j] == '%' {
                expand_number(&int_part, "", false, out);
                push_words(out, ["percent".to_string()]);
                i = j + 1;
                continue;
            }
            let mut k = j;
            while k < chars.len() && chars[k].is_ascii_alphabetic() {
                k += 1;
            }
            let suffix: String = chars[j..k].iter().collect();
            expand_number(&int_part, &suffix, allow_year, out);
            i = k;
            continue;
        }
        if c == '$' {
            let negative = matches!(chars.get(i + 1), Some('-' | '\u{2212}'))
                && chars.get(i + 2).is_some_and(|c| c.is_ascii_digit());
            let start = i + 1 + usize::from(negative);
            let mut j = start;
            while j < chars.len() && (chars[j].is_ascii_digit() || chars[j] == ',') {
                j += 1;
            }
            let amount: String = chars[start..j].iter().filter(|c| **c != ',').collect();
            if let Ok(n) = amount.parse::<u64>() {
                let mut cents = None;
                let frac_len = chars[j..]
                    .iter()
                    .skip(1)
                    .take_while(|c| c.is_ascii_digit())
                    .count();
                if chars.get(j) == Some(&'.') && (1..=2).contains(&frac_len) {
                    let digits: String = chars[j + 1..j + 1 + frac_len].iter().collect();
                    let value: u64 = digits.parse().unwrap_or(0);
                    cents = Some(if frac_len == 1 { value * 10 } else { value });
                    j += 1 + frac_len;
                }
                let mut words = Vec::new();
                if negative {
                    words.push("minus".into());
                }
                if n > 0 || cents.is_none_or(|c| c == 0) {
                    words.extend(number_words(n));
                    words.push(if n == 1 { "dollar" } else { "dollars" }.to_string());
                }
                if let Some(c) = cents.filter(|&c| c > 0) {
                    if n > 0 {
                        words.push("and".into());
                    }
                    words.extend(number_words(c));
                    words.push(if c == 1 { "cent" } else { "cents" }.to_string());
                }
                push_words(out, words);
                i = j;
            } else {
                push_words(out, ["dollars".to_string()]);
                i += 1;
            }
            continue;
        }
        if c.is_ascii_alphabetic() {
            let start = i;
            while i < chars.len() && is_word_char(chars[i]) {
                i += 1;
            }
            let word: String = chars[start..i].iter().collect();
            let lower = word.to_ascii_lowercase();
            let is_abbrev = ABBREVIATIONS.iter().any(|(a, _)| *a == lower);
            if is_abbrev && i < chars.len() && chars[i] == '.' {
                i += 1;
            }
            expand_word(&word, out);
            continue;
        }
        match c {
            '.' => {
                // An ellipsis or a run of periods is one break.
                while i + 1 < chars.len() && chars[i + 1] == '.' {
                    i += 1;
                }
                push_break(out, Break::Period);
            }
            '!' => push_break(out, Break::Exclamation),
            '?' => push_break(out, Break::Question),
            ',' | ';' | ':' | '(' | ')' | '\u{2014}' | '\u{2013}' => push_break(out, Break::Comma),
            '-' if i + 1 < chars.len() && chars[i + 1] == '-' => {
                push_break(out, Break::Comma);
                i += 1;
            }
            '&' => push_words(out, ["and".to_string()]),
            '%' => push_words(out, ["percent".to_string()]),
            '+' => push_words(out, ["plus".to_string()]),
            '=' => push_words(out, ["equals".to_string()]),
            '@' => push_words(out, ["at".to_string()]),
            _ => {}
        }
        i += 1;
    }
}

/// Ends the last clause as a statement, ahead of any trailing marks.
pub fn end_utterance(tokens: &mut Vec<Token>) {
    let Some(last) = tokens.iter().rposition(|t| !matches!(t, Token::Mark(_))) else {
        return;
    };
    match tokens[last] {
        Token::Word(_) => tokens.insert(last + 1, Token::Break(Break::Period)),
        Token::Break(Break::Comma) => tokens[last] = Token::Break(Break::Period),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &str) -> String {
        show(normalize(text))
    }

    fn show(tokens: Vec<Token>) -> String {
        tokens
            .into_iter()
            .map(|t| match t {
                Token::Word(w) => w.to_lowercase(),
                Token::Break(Break::Comma) => ",".into(),
                Token::Break(Break::Period) => ".".into(),
                Token::Break(Break::Question) => "?".into(),
                Token::Break(Break::Exclamation) => "!".into(),
                Token::Mark(id) => format!("<{id}>"),
            })
            .collect::<Vec<_>>()
            .join(" ")
    }

    #[test]
    fn utterance_ends_before_trailing_marks() {
        for (fragments, expected) in [
            (vec!["one two"], "one two ."),
            (vec!["one,"], "one ."),
            (vec!["one", "<1>", "two", "<2>"], "one <1> two . <2>"),
            (vec!["one,", "<1>"], "one . <1>"),
            (vec!["<1>"], "<1>"),
        ] {
            let mut tokens = Vec::new();
            for f in fragments {
                match f.strip_prefix('<').and_then(|f| f.strip_suffix('>')) {
                    Some(id) => tokens.push(Token::Mark(id.parse().unwrap())),
                    None => tokens.extend(normalize_fragment(f)),
                }
            }
            end_utterance(&mut tokens);
            assert_eq!(show(tokens), expected);
        }
    }

    #[test]
    fn numbers() {
        assert_eq!(words("42"), "forty two .");
        assert_eq!(
            words("1,234,567"),
            "one million two hundred thirty four thousand five hundred sixty seven ."
        );
        assert_eq!(words("3.14"), "three point one four .");
        assert_eq!(words("1984"), "nineteen eighty four .");
        assert_eq!(words("2005"), "two thousand five .");
        assert_eq!(words("21st"), "twenty first .");
        assert_eq!(words("1990s"), "nineteen nineties .");
        assert_eq!(words("$5"), "five dollars .");
        assert_eq!(
            words("$5.25 each"),
            "five dollars and twenty five cents each ."
        );
        assert_eq!(words("$0.5"), "fifty cents .");
        assert_eq!(
            words("18446744073709551616cats"),
            "one eight four four six seven four four zero seven three seven zero nine five five one six one six cats ."
        );
        assert_eq!(words("50%"), "fifty percent .");
    }

    #[test]
    fn punctuation_and_abbreviations() {
        assert_eq!(
            words("Dr. Smith, hello! Is it?"),
            "doctor smith , hello ! is it ?"
        );
        assert_eq!(words("Wait..."), "wait .");
        assert_eq!(words("BBC & TV"), "bee bee see and tee vee .");
        assert_eq!(words("don't"), "don't .");
    }

    #[test]
    fn typographic_apostrophes_preserve_words() {
        for apostrophe in ['\u{2018}', '\u{2019}', '\u{02bc}', '\u{ff07}'] {
            for word in ["don't", "Klatt's", "we're", "dogs'"] {
                assert_eq!(
                    normalize(&word.replace('\'', &apostrophe.to_string())),
                    normalize(word)
                );
            }
        }
        assert_eq!(normalize("\u{2018}hello\u{2019}"), normalize("'hello'"));
    }

    #[test]
    fn grouped_numbers_are_quantities_not_years() {
        assert_eq!(
            words("1,234 people"),
            "one thousand two hundred thirty four people ."
        );
        assert_eq!(words("1,984"), "one thousand nine hundred eighty four .");
        assert_eq!(words("1,234th"), "one thousand two hundred thirty fourth .");
        assert_eq!(
            words("1,234%"),
            "one thousand two hundred thirty four percent ."
        );
        assert_eq!(
            words("1,234.5"),
            "one thousand two hundred thirty four point five ."
        );
        assert_eq!(
            words("1,234cats"),
            "one thousand two hundred thirty four cats ."
        );
        assert_eq!(words("1984"), "nineteen eighty four .");
        assert_eq!(words("007"), "zero zero seven .");
    }

    #[test]
    fn negative_numbers_keep_the_sign() {
        assert_eq!(words("-5"), "minus five .");
        assert_eq!(
            words("it is -3.5%"),
            "it is minus three point five percent ."
        );
        assert_eq!(
            words("-1,234"),
            "minus one thousand two hundred thirty four ."
        );
        assert_eq!(
            words("-1234"),
            "minus one thousand two hundred thirty four ."
        );
        assert_eq!(words("\u{2212}5"), "minus five .");
        assert_eq!(words("well-known"), "well known .");
        assert_eq!(
            words("1984-1985"),
            "nineteen eighty four nineteen eighty five ."
        );
        assert_eq!(words("wait--5"), "wait , five .");
    }

    #[test]
    fn negative_currency_keeps_the_sign_and_units() {
        for input in ["-$5.25", "$-5.25", "\u{2212}$5.25", "$\u{2212}5.25"] {
            assert_eq!(
                words(input),
                "minus five dollars and twenty five cents .",
                "{input}"
            );
        }
        assert_eq!(words("-$0.01"), "minus one cent .");
        assert_eq!(words("$-1"), "minus one dollar .");
        assert_eq!(
            words("balance: -$1,234.50"),
            "balance , minus one thousand two hundred thirty four dollars and fifty cents ."
        );
        assert_eq!(words("$5.25"), "five dollars and twenty five cents .");
    }
}
