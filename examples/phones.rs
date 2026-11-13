use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut text = String::new();
    std::io::stdin().read_to_string(&mut text)?;
    for token in klatter::text::normalize(&text) {
        if let klatter::text::Token::Word(word) = token {
            let phones = klatter::utterance::pronounce(&word);
            let names: Vec<_> = phones
                .iter()
                .map(|(ph, stress)| {
                    if ph.is_vowel() {
                        format!("{}{stress}", ph.name())
                    } else {
                        ph.name().into()
                    }
                })
                .collect();
            println!("{word}: {}", names.join(" "));
        }
    }
    Ok(())
}
