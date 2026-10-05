# Klatter - A modern implementation of Dennis Klatt's 1980 TTS engine

This is a modern TTS engine written in Rust, based on Dennis Klatt's 1980's research.

The core engine is based on the [original 1980 paper](https://www.fon.hum.uva.nl/david/ma_ssp/2009/klatt-1980-JAS000971.pdf) and the [broader 1987 architectural paper](https://sail.usc.edu/~lgoldste/Ling582/Week%2013/Klatt1987.pdf). The paper is based on these papers, without reusing existing source code.

It was then improved based on [this 1990 paper](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1990-JAS000820.pdf).

It was then improved using Susan Hertz's papers, whose work also underlies the ETI-Eloquence engine:

- [English text to speech rules with Delta (1986)](https://www.academia.edu/56835165/English_text_to_speech_conversion_with_delta): splitting words into roots and suffixes before letter-to-sound conversion.
- [A nucleus-based timing model applied to multi-dialect speech synthesis by rule (1992)](https://www.isca-archive.org/icslp_1992/hertz92_icslp.pdf): timing the vowel and its following liquids and glides as one nucleus, and shortening the voiced vowel after aspiration.
- [Language-universal and language-specific components in the multi-language ETI-Eloquence text-to-speech system (1999)](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf): transition timings, the timing inside AY, AW and OY, and where the accent peak falls.

In addition to the engine, there is a command-line tool which accepts two parameters: in.txt and out.wav.

## Building and running

```
cargo build --release
target\release\klatter in.txt out.wav
```

The output is a 16 kHz, 16-bit mono WAV file. The library function `klatter::synthesize(text, sample_rate)` also accepts other sample rates. The project uses no third-party crates.

## Pipeline

1. `text.rs` normalizes the text. It expands numbers, years, ordinals, money, percentages and common abbreviations, spells out acronyms, and splits clauses at punctuation.
2. `lexicon.rs` holds an exceptions dictionary. Words not in it are tried by `morph.rs`, which strips the suffixes -s, -ed, -ing and -ly, repairs the root's spelling (for example hopping to hop, hoped to hope), and adds a suffix pronunciation that matches the root's last sound. Everything else goes to `lts.rs`, which applies letter-to-sound rules and stress heuristics.
3. `utterance.rs` syllabifies each word and marks function words and phrase-final syllables.
4. `prosody.rs` applies Klatt's segment duration rules. A vowel and the liquids and glides after it in the same syllable are timed as one nucleus, so a longer coda leaves less time for the vowel. Aspiration after a voiceless stop lengthens the syllable while shortening the voiced vowel. The pitch contour is a hat pattern with declination, and each accent peaks about three quarters of the way through its nucleus.
5. `params.rs` turns the phonemes into 5 ms parameter frames using synthesis-by-rule targets and transitions. Formant transitions last about 30 ms after fricatives, run through the aspiration after aspirated stops, and close quickly after an offglide. AY, AW and OY have a fixed 80 ms glide and a brief offglide. Following the 1990 paper, vowels become breathier toward an offset into silence, next to voiceless consonants, and in unstressed or phrase-final syllables. Breathier voicing means a higher open quotient, more spectral tilt, aspiration noise and wider first and second formant bandwidths. Voicing also weakens by about 10 dB as it fades into silence.
6. `klatt.rs` contains KLSYN88, the cascade/parallel formant synthesizer as described in the 1990 paper. The KLGLOTT88 voicing source runs at four times the output rate so pitch periods can be a fraction of a sample long. It has open quotient, a spectral tilt filter, flutter and diplophonia controls. The first formant can change while the glottis is open, and a tracheal pole-zero pair can be added.
7. `wav.rs` writes the WAV file.

## Known limitations

- Letter-to-sound rules guess wrong on some irregular words. You can add those words to the dictionary in `lexicon.rs`.
- There is no emphasis markup, and phrasing comes from punctuation only.
- The input must be English.
