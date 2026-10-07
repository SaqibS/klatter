//! Dumps the segment timeline the engine builds for a text, as TSV on stdout.
//!
//! Columns: ph, stress, fn (function word), final (phrase-final), start_ms,
//! dur_ms (frame-quantized, as rendered), f1-f3 (table targets), wi (word
//! initial), word (the word spelled out on its first phone). This is the
//! `segs.tsv` input for tools/eloquence.
//!
//! Usage:
//!   cargo run --release --example segdump -- text.txt > segs.tsv
//!   cargo run --release --example segdump -- text.txt SCALE out.wav > segs.tsv
//!
//! With SCALE, every duration is multiplied by it and the result is also
//! rendered to out.wav (16 kHz), e.g. to match a reference speaking rate.
use klatter::*;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let text = std::fs::read_to_string(&args[1]).unwrap();
    let tokens = text::normalize(&text);
    let words: Vec<String> = tokens
        .iter()
        .filter_map(|t| match t {
            text::Token::Word(w) => Some(w.clone()),
            _ => None,
        })
        .collect();
    let mut word_iter = words.iter();
    let mut segs = utterance::build(&tokens);
    prosody::assign_durations(&mut segs);

    if let Some(scale) = args.get(2).map(|s| s.parse::<f64>().unwrap()) {
        for seg in &mut segs {
            seg.dur_ms *= scale;
        }
        let frames = params::build_frames(&segs);
        let raw = klatt::Synth::new(16000).render(&frames);
        let peak = raw.iter().fold(0.0f64, |m, x| m.max(x.abs()));
        let pcm: Vec<i16> = raw.iter().map(|x| (x / peak * 29000.0) as i16).collect();
        wav::write_wav(std::fs::File::create(&args[3]).unwrap(), &pcm, 16000).unwrap();
    }

    let (starts, lens) = params::frame_layout(&segs);
    println!("ph\tstress\tfn\tfinal\tstart_ms\tdur_ms\tf1\tf2\tf3\twi\tword");
    for (i, seg) in segs.iter().enumerate() {
        let (name, f) = match seg.ph {
            Some(p) => (p.name(), p.spec().f),
            None => ("_", [0.0; 3]),
        };
        let word = if seg.word_initial && seg.ph.is_some() {
            word_iter.next().map_or("?", |w| w.as_str())
        } else {
            ""
        };
        println!(
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
        );
    }
}
