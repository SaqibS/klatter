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
| [Klatt (1987), *Review of text-to-speech conversion for English*](https://sail.usc.edu/~lgoldste/Ling582/Week%2013/Klatt1987.pdf) | Segment duration rules (the `MIN + (INH - MIN) x PRCNT` model), locus theory, aspiration and voice onset time, phrase-final lengthening, and intonation. The numeric inherent and minimum durations are Klatt's (1979) values as summarized there, except for some vowels (see below). |
| [Klatt & Klatt (1990), *Analysis, synthesis, and perception of voice quality variations among female and male talkers*](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1990-JAS000820.pdf) | The KLSYN88 synthesizer and its KLGLOTT88 voicing source: open quotient, spectral tilt, flutter, double pulsing, open-phase F1 changes, and breathy phonation toward voiceless consonants and silence. |
| [Hertz, Kadin & Karplus (1985), *The Delta rule development system for speech synthesis from text*](https://users.soe.ucsc.edu/~karplus/papers/delta-85.pdf) | Representing speech as coordinated streams, and splitting words into roots and suffixes before letter-to-sound conversion. |
| Hertz (1991), *Streams, phones and transitions: toward a new phonological and phonetic model of formant timing*, Journal of Phonetics 19, 91-109 | The phone-and-transition timing model: formant transitions as timed units of their own between phones, with durations that depend on the two phones (stop, fricative, vowel, liquid and offglide joins); transitions that stay stable when syllables are lengthened, so only steady states stretch; aspiration spanning the release transition; and transitions that shorten in fast speech in a different proportion from phones. |
| [Hertz & Huffman (1992), *A nucleus-based timing model applied to multi-dialect speech synthesis by rule*](https://www.isca-archive.org/icslp_1992/hertz92_icslp.pdf) | Timing a vowel and its following liquids and glides as one nucleus, in which the vowel takes whatever time the transitions and other phones leave, and shortening the voiced vowel after aspiration. |
| [Hertz, Younes & Zinovieva (1999), *Language-universal and language-specific components in the multi-language ETI-Eloquence text-to-speech system*](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf) | Phones joined by explicit transitions, example transition timings, voicing and aspiration aligned with phone and transition edges, diphthong glide timing, and accent-peak placement. This is the published description of Eloquence's own design. |
| Fry (1955), *Duration and intensity as physical correlates of linguistic stress*, JASA 27(4), 765-768; Lehiste (1970), *Suprasegmentals* | Greater intensity as a correlate of stress. |
| Löfqvist & McGowan (1992), *Influence of consonantal environment on voice source aerodynamics*, Journal of Phonetics 20 | Breathy voicing at vowel onsets after voiceless consonants that changes gradually into modal voicing, so voicing grows after aspiration rather than switching on. |
| Ladefoged & Johnson (2011), *A Course in Phonetics*, 6th ed. | American English connected speech: weak forms of function words, and flapping of /t/ and /d/ between a vowel and an unstressed vowel. |
| Zue & Laferriere (1979), *Acoustic study of medial /t,d/ in American English*, JASA 66(4), 1039-1050 | Flaps as brief voiced taps without a release burst. |
| Ladefoged & Maddieson (1996), *The Sounds of the World's Languages* | Taps and flaps as a single ballistic movement of the tongue tip. |
| Flemming & Johnson (2007), *Rosa's roses: reduced vowels in American English*, JIPA 37(1), 83-96 | Two reduced vowels: a high central [ɨ] between consonants and [ə] elsewhere. |

Modern neural and hybrid formant research ([KLATTSTAT, 2010](https://www.isca-archive.org/ssw_2010/anumanchipalli10b_ssw.pdf); [speaker-independent neural formant synthesis, 2023](https://arxiv.org/abs/2306.01957); [HiFi-Glot](https://arxiv.org/abs/2409.14823); [ARIS](https://arxiv.org/abs/2609.29923)) was read as background. It was not adopted: learned models trade determinism, latency and behaviour at extreme speaking rates for naturalness, and screen-reader users depend on the former.

### Where Klatter departs from the papers

Some values are not taken from the papers. They are deliberate and documented in the code:

- **Even rhythm.** Stressed vowels are shorter and unstressed vowels in content words longer than Klatt's rules give, for evenly timed syllables (except in phrase-final syllables, which are already lengthened; unstressed vowels in a word's last syllable get less extra length, and a schwa before a syllabic /l/ none). Vowels are also spread less by quality than in Klatt's table, so AA, AO, AY and UH are shorter and IY, IH and EY longer.
- **Diphthong timing.** AY, AW and OY hold their open onset from the end of the transition into them and glide in about 55 ms just before the next consonant; before a voiceless obstruent they glide from a quarter of the way through.
- **Faster transitions.** Formant transitions take about half the durations of Hertz's published examples, so that short vowels reach their targets instead of drifting toward their neighbours. Hertz (1991) says transitions and phones shorten in different proportions in fast speech but gives no figures, so above the natural rate transitions shorten as the 1.33 power of the duration scale, which keeps vowels' quality at high rates. Joins the papers do not time (into nasals, from vowels into fricatives, between glides and vowels) keep Klatter's earlier durations.
- **Vowel and nasal targets.** Vowel formants and bandwidths, nasal formants and relative loudness are tuned rather than taken from the papers. The two reduced vowels never occur sustained, so they are tuned for running speech.
- **Flaps.** Transitions into and out of a flap are capped at 20 ms, so it interrupts the vowels only briefly.
- **Velar loci.** /k/ and /g/ take their F2 locus from the neighbouring vowel, following a locus equation.
- **Brighter voicing.** A gentle presence boost on voicing only, and fixed upper formants placed to keep energy at 3.5-5 kHz, give Klatter more high-frequency clarity.

Klatter's voice is male, so its voice-quality and stress-intensity rules follow Klatt & Klatt's (1990) measurements of male talkers, which in places differ from their summary across all talkers:

- **Modal unstressed syllables.** Klatt & Klatt (1990) summarize breathiness as greater in unstressed syllables, but their male talkers' first harmonic at mid-vowel was no stronger in unstressed syllables (Table II) and their vowels carried little aspiration noise (Tables V and VI). Klatter keeps unstressed syllables modal and, as in Klatt's (1987) rules, reserves breathiness for phrase-final syllables, offsets into silence and the margins of voiceless consonants.
- **Short voiceless margins.** The breathy margins beside voiceless consonants follow the 80 ms ramps of Klatt & Klatt's Figs. 15 and 16 but stay within the outer third of a sonorant, because their Table II shows that even [h] does not make the middle of the vowel breathier.
- **Weaker unstressed vowels.** Stressed syllables are more intense (Fry 1955; Lehiste 1970), by 4.3 dB for Klatt & Klatt's male talkers, partly through their higher F0. Klatt (1987) warned that a large stress-intensity rule makes stressed vowels sound artificially strong, so Klatter voices unstressed vowels 3 dB more weakly, which with the pitch contour gives stressed vowels about 3.5 dB more intensity than the same vowel unstressed.

### Pipeline

1. **Text** (`text.rs`) expands numbers, abbreviations, acronyms and identifiers, folds accents, and splits clauses at punctuation, keeping each word's original UTF-8 position.
2. **Pronunciation** (`lexicon.rs`, `morph.rs`, `lts.rs`) tries caller overrides, a small built-in table, then CMUdict. Words spelled -day end in a full [deɪ] ("Tuesday", "Saturday"), the first reading in American dictionaries such as Merriam-Webster, where CMUdict lists the reduced [di] first for some weekday names. Unknown words go through unstressed-prefix analysis (un-, re-, dis- and mis- before a dictionary word, as in "untitled"), suffix analysis, conservative compound splitting, and finally letter-to-sound rules with stress heuristics.
3. **Structure** (`utterance.rs`) syllabifies words, marks function words and phrase-final syllables, and applies American connected-speech forms. Function words take weak forms ("to" [tə], "for" [fɚ]) when another word follows in the phrase, and keep their strong forms when spoken alone or at the end of a phrase ("what is it for?"). Unstressed vowels reduce to [ɨ] between consonants ("rabbit") and to [ə] elsewhere ("about", "little"), except in "-ing". /t/ and /d/ flap after a vowel or /r/, before an unstressed vowel in the word ("water") or any vowel across a word boundary in the phrase ("get up"). Caller overrides keep their vowels as written; flapping applies to them too. A listener heard Klatter without these forms as Scottish- or Irish-accented.
4. **Prosody** (`prosody.rs`, `timing.rs`) applies the duration rules, nucleus timing and aspiration. Phones and formant transitions are then timed as separate units: each transition's duration depends on the two phones it joins, a phone's steady state is what its segment leaves after the transitions inside it, and within a syllable nucleus the vowel gives up the time. Prosody then builds the pitch contour: a hat pattern with declination, accent peaks about three quarters of the way through the nucleus, boundary tones for statements, questions and commas, and consonant microprosody. Pauses are 165 ms at commas and 385 ms at sentence ends at the natural rate, and shrink faster than speech as the rate rises.
5. **Acoustics** (`params.rs`) turns phonemes into 5 ms parameter frames: formant targets held through each phone's steady state and interpolated across the timed transitions, stop closures with a low voice bar, flaps as brief voiced taps, bursts, aspiration, frication, nasalization, and voicing and noise cross-fades so sounds are not spliced together.
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
| `phones` | Prints each word's dictionary or letter-to-sound pronunciation, before connected-speech forms (`cargo run --release --example phones < text.txt`). |
| `segdump` | Prints the segment timeline as spoken: phones, stress, flaps, durations and targets (`cargo run --release --example segdump -- text.txt`). With `--frames`, it prints each 5 ms frame instead: its segment and word, the voice-source parameters (AV, AH, OQ, TL, B1, F0), the rendered level, the formants and the noise and parallel amplitudes. |
| `lts_accuracy` | Measures letter-to-sound agreement with CMUdict (`cargo run --release --example lts_accuracy`). |

The Rust diagnostics live in `tools` and are built as Cargo examples, so they are not part of the CLI, DLL or add-on. Reference recordings are not included.

## Known limitations

- English only.
- Pronunciation does not use sentence context, so homographs such as present- and past-tense *read* take one reading.
- Phrasing comes from punctuation only, and there is no emphasis markup.
- Phones and transitions are timed separately, but Klatt's segment durations still set the time they share, rather than Hertz's own duration rules. Hertz's further streams (separate timing per acoustic parameter, voicing that stops partway through a transition before a voiceless consonant, and diphthong glides cut short in fast speech) are not modelled.

## Licensing

Klatter is licensed under the terms of the MIT license.

The CMU Pronouncing Dictionary is distributed under its own BSD-style licence, in `data/cmudict/LICENSE.txt`, which must accompany binary distributions (the NVDA builder includes it).
