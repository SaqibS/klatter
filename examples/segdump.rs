//! Dumps the segment timeline the engine builds for a text, as TSV on stdout.
//!
//! Columns: ph, stress, fn (function word), final (phrase-final), start_ms,
//! dur_ms (frame-quantized, as rendered), f1-f3 (table targets), wi (word
//! initial), word (the word spelled out on its first phone).
//!
//! Usage:
//!   cargo run --release --example segdump -- text.txt > segs.tsv
//!   cargo run --release --example segdump -- text.txt SCALE out.wav > segs.tsv
//!
//! With SCALE, every duration is multiplied by it and the result is also
//! rendered to out.wav (16 kHz), e.g. to match a reference speaking rate.
use klatter::*;
use std::io::{self, BufWriter, Write};
use std::process::ExitCode;

fn parse_args(args: &[String]) -> Option<(&str, Option<(f64, &str)>)> {
    match args {
        [input] => Some((input, None)),
        [input, scale, output] => {
            let scale = scale
                .parse::<f64>()
                .ok()
                .filter(|s| s.is_finite() && *s > 0.0)?;
            Some((input, Some((scale, output))))
        }
        _ => None,
    }
}

fn run(input: &str, render: Option<(f64, &str)>) -> io::Result<()> {
    let text = std::fs::read_to_string(input)?;
    let tokens = text::normalize(&text);
    let words: Vec<&str> = tokens
        .iter()
        .filter_map(|t| match t {
            text::Token::Word(w) => Some(w.as_str()),
            _ => None,
        })
        .collect();
    let mut word_iter = words.iter();
    let mut segs = utterance::build(&tokens);
    prosody::assign_durations(&mut segs);

    if let Some((scale, _)) = render {
        prosody::scale_durations(&mut segs, scale);
    }
    let (starts, lens) = params::frame_layout(&segs);
    if let Some((_, output)) = render {
        let frames = params::build_frames_with_layout(&segs, &starts, &lens);
        let raw = klatt::Synth::new(16000).render(&frames);
        let peak = raw.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        let gain = if peak > 0.0 { 29000.0 / peak } else { 0.0 };
        let pcm: Vec<i16> = raw.iter().map(|x| (x * gain) as i16).collect();
        wav::write_wav(BufWriter::new(std::fs::File::create(output)?), &pcm, 16000)?;
    }

    let mut out = BufWriter::new(io::stdout().lock());
    writeln!(
        out,
        "ph\tstress\tfn\tfinal\tstart_ms\tdur_ms\tf1\tf2\tf3\twi\tword"
    )?;
    for (i, seg) in segs.iter().enumerate() {
        let (name, f) = match seg.ph {
            Some(p) => (p.name(), p.spec().f),
            None => ("_", [0.0; 3]),
        };
        let word = if seg.word_initial && seg.ph.is_some() {
            word_iter.next().copied().unwrap_or("?")
        } else {
            ""
        };
        writeln!(
            out,
            "{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{word}",
            seg.stress,
            seg.function_word as u8,
            seg.phrase_final as u8,
            starts[i] as f64 * klatt::FRAME_MS,
            lens[i] as f64 * klatt::FRAME_MS,
            f[0],
            f[1],
            f[2],
            seg.word_initial as u8,
        )?;
    }
    out.flush()
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let Some((input, render)) = parse_args(&args) else {
        eprintln!("usage: segdump <text.txt> [SCALE <out.wav>]");
        return ExitCode::from(2);
    };
    match run(input, render) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("segdump: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_complete_arguments_and_positive_finite_scales() {
        for args in [
            vec![],
            vec!["text.txt", "2"],
            vec!["text.txt", "0", "out.wav"],
            vec!["text.txt", "-1", "out.wav"],
            vec!["text.txt", "nan", "out.wav"],
            vec!["text.txt", "inf", "out.wav"],
            vec!["text.txt", "fast", "out.wav"],
            vec!["text.txt", "2", "out.wav", "extra"],
        ] {
            let args: Vec<String> = args.into_iter().map(String::from).collect();
            assert!(parse_args(&args).is_none(), "{args:?}");
        }
        assert_eq!(parse_args(&["text.txt".into()]), Some(("text.txt", None)));
        assert_eq!(
            parse_args(&["text.txt".into(), "0.5".into(), "out.wav".into()]),
            Some(("text.txt", Some((0.5, "out.wav"))))
        );
    }
}
