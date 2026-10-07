use std::fs::{self, File};
use std::io::BufWriter;
use std::process::ExitCode;

use klatter::{DEFAULT_SAMPLE_RATE, DEFAULT_WPM, Item, Voice, speak, wav};

/// Accepts say(1)'s `-r rate` and `--rate=rate`, in words per minute.
fn parse_args(args: &[String]) -> Option<(&str, &str, f64)> {
    let (rate, files) = match args {
        [flag, rate, rest @ ..] if flag == "-r" => (Some(rate.as_str()), rest),
        [flag, rest @ ..] if flag.starts_with("--rate=") => (Some(&flag["--rate=".len()..]), rest),
        _ => (None, args),
    };
    let wpm = match rate {
        Some(r) => r
            .parse::<f64>()
            .ok()
            .filter(|w| w.is_finite() && *w > 0.0)?,
        None => DEFAULT_WPM,
    };
    match files {
        [input, output] if !input.starts_with('-') => Some((input, output, wpm)),
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((input, output, wpm)) = parse_args(&args) else {
        eprintln!("usage: klatter [-r rate] <in.txt> <out.wav>");
        return ExitCode::from(2);
    };
    let text = match fs::read_to_string(input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let samples = speak(
        &[Item::Text(&text)],
        DEFAULT_SAMPLE_RATE,
        &Voice {
            wpm,
            ..Voice::default()
        },
    )
    .samples;
    let result = File::create(output)
        .and_then(|f| wav::write_wav(BufWriter::new(f), &samples, DEFAULT_SAMPLE_RATE));
    if let Err(e) = result {
        eprintln!("error: cannot write {output}: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(args: &[&str]) -> Option<f64> {
        parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).map(|(_, _, wpm)| wpm)
    }

    #[test]
    fn rate_follows_say_syntax() {
        assert_eq!(parse(&["in.txt", "out.wav"]), Some(DEFAULT_WPM));
        assert_eq!(parse(&["-r", "300", "in.txt", "out.wav"]), Some(300.0));
        assert_eq!(parse(&["--rate=150.5", "in.txt", "out.wav"]), Some(150.5));
        for args in [
            vec![],
            vec!["in.txt"],
            vec!["-r", "300"],
            vec!["-r", "0", "in.txt", "out.wav"],
            vec!["-r", "-5", "in.txt", "out.wav"],
            vec!["-r", "fast", "in.txt", "out.wav"],
            vec!["--rate=", "in.txt", "out.wav"],
            vec!["-r", "inf", "in.txt", "out.wav"],
            vec!["in.txt", "out.wav", "-r", "300"],
            vec!["-x", "in.txt", "out.wav"],
        ] {
            assert_eq!(parse(&args), None, "{args:?}");
        }
    }
}
