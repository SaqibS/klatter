use std::fs::{self, File};
use std::io::BufWriter;
use std::process::ExitCode;

use klatter::{DEFAULT_SAMPLE_RATE, synthesize, wav};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let [input, output] = args.as_slice() else {
        eprintln!("usage: klatter <in.txt> <out.wav>");
        return ExitCode::from(2);
    };
    let text = match fs::read_to_string(input) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("error: cannot read {input}: {e}");
            return ExitCode::FAILURE;
        }
    };
    let samples = synthesize(&text, DEFAULT_SAMPLE_RATE);
    let result = File::create(output)
        .and_then(|f| wav::write_wav(BufWriter::new(f), &samples, DEFAULT_SAMPLE_RATE));
    if let Err(e) = result {
        eprintln!("error: cannot write {output}: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
