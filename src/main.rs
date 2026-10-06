use std::fs::{self, File};
use std::io::BufWriter;
use std::process::ExitCode;

use klatter::{DEFAULT_SAMPLE_RATE, params::Timing, synthesize_with_timing, wav};

fn parse_args(args: &[String]) -> Option<(&str, &str, Timing)> {
    match args {
        [input, output] if !input.starts_with("--") => Some((input, output, Timing::Original)),
        [flag, value, input, output] if flag == "--timing" && !input.starts_with("--") => {
            let timing = match value.as_str() {
                "original" => Timing::Original,
                "compact-holds" => Timing::CompactHolds,
                _ => return None,
            };
            Some((input, output, timing))
        }
        _ => None,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((input, output, timing)) = parse_args(&args) else {
        eprintln!("usage: klatter [--timing original|compact-holds] <in.txt> <out.wav>");
        return ExitCode::from(2);
    };
    let text = match fs::read_to_string(input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let samples = synthesize_with_timing(&text, DEFAULT_SAMPLE_RATE, timing);
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

    #[test]
    fn timing_arguments_are_explicit_and_strict() {
        let parse = |args: &[&str]| {
            parse_args(&args.iter().map(|s| s.to_string()).collect::<Vec<_>>()).map(|(_, _, t)| t)
        };
        assert_eq!(parse(&["in.txt", "out.wav"]), Some(Timing::Original));
        for (name, timing) in [
            ("original", Timing::Original),
            ("compact-holds", Timing::CompactHolds),
        ] {
            assert_eq!(
                parse(&["--timing", name, "in.txt", "out.wav"]),
                Some(timing)
            );
        }
        for args in [
            vec![],
            vec!["in.txt"],
            vec!["--timing", "compact-holds"],
            vec!["--timing", "unknown", "in.txt", "out.wav"],
            vec!["--other", "compact-holds", "in.txt", "out.wav"],
            vec!["in.txt", "out.wav", "extra"],
            vec![
                "--timing",
                "original",
                "--timing",
                "compact-holds",
                "in.txt",
                "out.wav",
            ],
            vec!["--timing"],
            vec!["in.txt", "out.wav", "--timing", "compact-holds"],
        ] {
            assert_eq!(parse(&args), None);
        }
    }
}
