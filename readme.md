# Klatter

Klatter is a formant text-to-speech engine for screen readers, written in Rust. It is a clean-room implementation built only from published research papers.

## Why Klatter?

Many blind screen-reader users have listened to ETI-Eloquence for decades and still prefer it to modern neural voices. The reasons are practical rather than aesthetic:

- **Intelligibility.** Every phoneme is rendered with its full set of acoustic cues, so words are easy to identify even out of context.
- **Compact articulation.** Short, crisp segments that do not smear together.
- **Clarity at high speaking rates.** Experienced users listen at 300 to 900 words per minute.
- **Comfortable prolonged listening.** A stable, predictable voice that is tolerable for a whole working day.
- **Responsiveness.** Speech starts and stops immediately as the user moves around an interface.

Eloquence is proprietary, no longer actively developed, and distributed as a 32-bit Windows library. Klatter asks whether an open, modern engine can deliver the same qualities by going back to the research that formant synthesizers like Eloquence grew out of: Dennis Klatt's synthesizer and rules, and Susan Hertz's work on timing and synthesis control.

Klatter aims for these qualities (see [How Klatter was built](#how-klatter-was-built)).

## Using Klatter

### Command line

```
cargo build --release
target\release\klatter in.txt out.wav
target\release\klatter -r 300 in.txt out.wav
```

`-r 300` (or `--rate=300`) sets the speaking rate in words per minute.

### NVDA add-on

`nvda/addon` contains a synthesizer driver for the NVDA screen reader. It supports rate, pitch, inflection and volume, index marks for cursor tracking, and mid-utterance pitch changes (NVDA uses these for capital letters). NVDA's 0-100 settings map onto Klatter's voice settings: 50 is Klatter's natural voice, and each 50 steps doubles or halves the rate or pitch.

```
rustup target add x86_64-pc-windows-msvc
python nvda\build.py
```

This writes `target\klatter-<version>.nvda-addon`. Open it to install, restart NVDA, and choose Klatter in the Speech settings. NVDA is a 64-bit x86 program, so the add-on carries an x64 DLL; it also runs under emulation on ARM64 Windows.

Stopping speech (for example, pressing Control) cancels the native utterance as well as stopping playback, so a long paragraph that is still being planned does not delay the next utterance.

### Rust library

```rust
use klatter::{speak, Item, Voice};

let voice = Voice { wpm: 350.0, ..Voice::default() };
let speech = speak(&[Item::Text("Link, visited."), Item::Mark(7)], 22050, &voice);
// speech.samples: i16 PCM; speech.marks: (7, sample index where the mark falls)
```

- `Voice` uses physical units: words per minute, a pitch baseline in Hz (100 by default; the whole contour moves with it), an inflection multiplier (0 is monotone, 1 natural) and a linear volume. Hosts map their own scales onto these.
- `Item`s interleave text with numbered marks. All items are spoken as one utterance, so a mark never breaks the phrasing.
- Any sample rate is supported.
- For screen readers, `Utterance` pulls audio into host-owned buffers of any size and reports word events with the original text position (including for expanded numbers and abbreviations). Each utterance has a cancel handle that can be used from another thread. The audio is identical regardless of buffer size.
- `Pronunciations` holds caller-owned overrides, such as `insert("read", "R EH1 D")`, which take precedence over the built-in dictionary.

### C interface

The `ffi` crate builds `klatter.dll`, declared in `ffi/klatter.h`.

- `klatter_speak` returns a whole utterance's samples and mark positions.
- `klatter_utterance_create` / `klatter_utterance_read` provide pull rendering with word events and a thread-safe cancel handle (`klatter_utterance_cancel_handle`, `klatter_cancel`).
- `klatter_pronunciations_*` manage pronunciation overrides.

Functions return null or a negative status for invalid input and never unwind a Rust panic into the caller. The header documents ownership and threading rules.

## How Klatter was built

### Clean-room approach

- **Literature only.** The engine was implemented from the published papers listed below. No source code from any other TTS engine was read or consulted at any stage.
- **Built with an AI coding agent.** Klatter was developed with GitHub Copilot CLI as an AI coding agent working under the author's direction. The agent wrote code and analysis tools, ran measurements and prepared listening tests, under the same literature-only rules. The author set the goals, judged every listening test and decided what to keep.

### Papers and how they were used

| Paper | Used for |
|---|---|
| [Klatt (1980), *Software for a cascade/parallel formant synthesizer*](https://www.fon.hum.uva.nl/david/ma_ssp/2009/klatt-1980-JAS000971.pdf) | The cascade/parallel synthesizer structure, resonator and antiresonator equations, the parameter set, and vowel and consonant formant targets. |
| [Klatt (1987), *Review of text-to-speech conversion for English*](https://sail.usc.edu/~lgoldste/Ling582/Week%2013/Klatt1987.pdf) | Segment duration rules (the `MIN + (INH - MIN) x PRCNT` model), locus theory, aspiration and voice onset time, phrase-final lengthening, and intonation. The numeric inherent and minimum durations are Klatt's (1979) values as summarized there. |
| [Klatt & Klatt (1990), *Analysis, synthesis, and perception of voice quality variations among female and male talkers*](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1990-JAS000820.pdf) | The KLSYN88 synthesizer and its KLGLOTT88 voicing source: open quotient, spectral tilt, flutter, double pulsing, open-phase F1 changes, and breathy phonation toward voiceless consonants and silence. |
| [Hertz, Kadin & Karplus (1985), *The Delta rule development system for speech synthesis from text*](https://users.soe.ucsc.edu/~karplus/papers/delta-85.pdf) | Representing speech as coordinated streams, and splitting words into roots and suffixes before letter-to-sound conversion. |
| [Hertz & Huffman (1992), *A nucleus-based timing model applied to multi-dialect speech synthesis by rule*](https://www.isca-archive.org/icslp_1992/hertz92_icslp.pdf) | Timing a vowel and its following liquids and glides as one nucleus, and shortening the voiced vowel after aspiration. |
| [Hertz, Younes & Zinovieva (1999), *Language-universal and language-specific components in the multi-language ETI-Eloquence text-to-speech system*](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf) | Phones joined by explicit transitions, example transition timings, diphthong glide timing, and accent-peak placement. This is the published description of Eloquence's own design. |

Modern neural and hybrid formant research ([KLATTSTAT, 2010](https://www.isca-archive.org/ssw_2010/anumanchipalli10b_ssw.pdf); [speaker-independent neural formant synthesis, 2023](https://arxiv.org/abs/2306.01957); [HiFi-Glot](https://arxiv.org/abs/2409.14823); [ARIS](https://arxiv.org/abs/2609.29923)) was read as background. It was not adopted: learned models trade determinism, latency and behaviour at extreme speaking rates for naturalness, and screen-reader users depend on the former.

### Where Klatter departs from the papers

Some values are not taken from the papers. They are deliberate and documented in the code:

- **Even rhythm.** Stressed vowels are shorter and unstressed vowels in content words longer than Klatt's rules give, for evenly timed syllables (except in phrase-final syllables, which are already lengthened).
- **Faster transitions.** Formant transitions take about half the durations of Hertz's published examples, so that short vowels reach their targets instead of drifting toward their neighbours.
- **Vowel and nasal targets.** Vowel formants and bandwidths, nasal formants and relative loudness are tuned rather than taken from the papers.
- **Velar loci.** /k/ and /g/ take their F2 locus from the neighbouring vowel, following a locus equation.
- **Brighter voicing.** A gentle presence boost on voicing only, and fixed upper formants placed to keep energy at 3.5-5 kHz, give Klatter more high-frequency clarity.

### Pipeline

1. **Text** (`text.rs`) expands numbers, abbreviations, acronyms and identifiers, folds accents, and splits clauses at punctuation, keeping each word's original UTF-8 position.
2. **Pronunciation** (`lexicon.rs`, `morph.rs`, `lts.rs`) tries caller overrides, a small built-in table, then CMUdict. Unknown words go through suffix analysis, conservative compound splitting, and finally letter-to-sound rules with stress heuristics.
3. **Structure** (`utterance.rs`) syllabifies words and marks function words and phrase-final syllables.
4. **Prosody** (`prosody.rs`) applies the duration rules, nucleus timing and aspiration, then builds the pitch contour: a hat pattern with declination, accent peaks about three quarters of the way through the nucleus, boundary tones for statements, questions and commas, and consonant microprosody. Pauses are 165 ms at commas and 385 ms at sentence ends at the natural rate, and shrink faster than speech as the rate rises.
5. **Acoustics** (`params.rs`) turns phonemes into 5 ms parameter frames: formant targets and transitions, stop closures with a low voice bar, bursts, aspiration, frication, nasalization, and voicing and noise cross-fades so sounds are not spliced together.
6. **Synthesis** (`klatt.rs`) is KLSYN88. The voicing source runs at four times the output rate, so pitch periods need not be whole samples, and formant frequencies glide sample by sample between frames so changing a ringing filter never clicks.
7. **Voice settings** (`lib.rs`) scale durations for rate and reshape the pitch contour for pitch and inflection.

### How changes were evaluated

Speech quality cannot be unit-tested, so every audible change was judged by listening, using tools in `tools/`:

- **Blind A/B tests, one pair at a time.** Both versions are calibrated to the same measured speaking rate and loudness, presented in random order, with the question spoken in the audio. Changes were kept only when preferred, or when they measurably helped without sounding worse.
- **Regression tests** pin behaviour that should not change: duration rules, number and text normalization, pronunciation fallbacks, exact equality of audio rendered in different buffer sizes and sample rates, cancellation, and the C interface.

## Development

```
cargo test --workspace --all-targets
cargo clippy --workspace --all-targets -- -D warnings
python -m unittest discover -s tools
python nvda\test_driver.py target\release\klatter.dll
```

The NVDA driver test uses stand-ins for NVDA's modules; the DLL must match the Python interpreter's architecture.

CMUdict is bundled as `data/cmudict/cmudict.dict` and is the single source of dictionary pronunciations. Cargo validates it and generates the embedded lookup table at build time, without Python or network access. Its pinned identity is in `data/cmudict/README.txt`.

### Tools

| Tool | Purpose |
|---|---|
| `tools/listen.py` | Rate- and loudness-matched blind A/B listening tests between two engines, with replay and a recorded manifest. Adapters in `tools/tts_engines.py` also support SAPI voices and eSpeak. |
| `tools/voice_metrics.py` | Compares a rendering with a reference recording using rate-independent measures: pitch distribution, H1-H2, formants, pauses and spectral balance. |
| `phones` | Prints the pronunciation Klatter chooses for each word (`cargo run --release --example phones < text.txt`). |
| `segdump` | Prints the segment timeline: phones, stress, durations and targets (`cargo run --release --example segdump -- text.txt`). |
| `lts_accuracy` | Measures letter-to-sound agreement with CMUdict (`cargo run --release --example lts_accuracy`). |

The Rust diagnostics live in `tools` and are built as Cargo examples, so they are not part of the CLI, DLL or add-on. Reference recordings are not included.

## Known limitations

- English only.
- Pronunciation does not use sentence context, so homographs such as present- and past-tense *read* take one reading.
- Phrasing comes from punctuation only, and there is no emphasis markup.
- Transitions occupy phoneme durations rather than being independently timed units, as they are in Hertz's model.

## Licensing

Klatter is licensed under the terms of the MIT license.

The CMU Pronouncing Dictionary is distributed under its own BSD-style licence, in `data/cmudict/LICENSE.txt`, which must accompany binary distributions (the NVDA builder includes it).
