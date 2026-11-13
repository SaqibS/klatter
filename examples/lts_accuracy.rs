use klatter::{lts, phoneme::Ph};

fn distance<T: PartialEq>(a: &[T], b: &[T]) -> usize {
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, left) in a.iter().enumerate() {
        let mut previous = row[0];
        row[0] = i + 1;
        for (j, right) in b.iter().enumerate() {
            let old = row[j + 1];
            row[j + 1] = (previous + usize::from(left != right))
                .min(row[j] + 1)
                .min(old + 1);
            previous = old;
        }
    }
    row[b.len()]
}

fn main() {
    let mut words = 0usize;
    let mut exact = 0usize;
    let mut errors = 0usize;
    let mut total = 0usize;
    for line in include_str!(concat!(env!("OUT_DIR"), "/cmudict.txt")).lines() {
        let mut parts = line.split_whitespace();
        let word = parts.next().expect("dictionary word");
        let reference: Vec<Ph> = parts
            .map(|p| Ph::parse(p.trim_end_matches(['0', '1', '2'])).expect("dictionary phone"))
            .collect();
        let rule: Vec<Ph> = lts::assign_stress(word, &lts::letters_to_phonemes(word))
            .into_iter()
            .map(|(ph, _)| ph)
            .collect();
        let difference = distance(&reference, &rule);
        words += 1;
        exact += usize::from(difference == 0);
        errors += difference;
        total += reference.len();
    }
    println!(
        "Raw letter-to-sound agreement with first CMUdict readings (no morphology or stress scoring):"
    );
    println!(
        "{words} words; {:.2}% exact; {:.2}% phoneme error",
        exact as f64 * 100.0 / words as f64,
        errors as f64 * 100.0 / total as f64
    );
}

#[cfg(test)]
mod tests {
    #[test]
    fn distance_counts_insertions_deletions_and_substitutions() {
        assert_eq!(super::distance(b"ABC", b"AC"), 1);
        assert_eq!(super::distance(b"AC", b"ABC"), 1);
        assert_eq!(super::distance(b"ABC", b"ADC"), 1);
        assert_eq!(super::distance(b"", b"A"), 1);
    }
}
