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

Klatter aims for these qualities. Its values come from the published literature, from neutral measurements such as speech-recognition intelligibility, or from blind listening between Klatter versions (see [How Klatter was built](#how-klatter-was-built)).

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
| [Klatt (1980), *Software for a cascade/parallel formant synthesizer*](https://www.fon.hum.uva.nl/david/ma_ssp/2009/klatt-1980-JAS000971.pdf) | The cascade/parallel synthesizer structure, resonator and antiresonator equations, the parameter set, the vowel, sonorant and consonant formant targets of Table III, and its consonant and sonorant bandwidths. Consonant amplitudes start from Table III but are set for Klatter's peak-normalized parallel branch, so they do not carry the table's values directly. |
| [Klatt (1987), *Review of text-to-speech conversion for English*](https://sail.usc.edu/~lgoldste/Ling582/Week%2013/Klatt1987.pdf) | Segment duration rules (the `MIN + (INH - MIN) x PRCNT` model), locus theory, aspiration and voice onset time, phrase-final lengthening, and intonation. Klatt (1987) gives the rules without numbers; the inherent and minimum durations and the rule percentages are Klatt's (1979), as used in MITalk (Allen, Hunnicutt & Klatt 1987), except for the vowels (see below). |
| [Klatt & Klatt (1990), *Analysis, synthesis, and perception of voice quality variations among female and male talkers*](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1990-JAS000820.pdf) | The KLSYN88 synthesizer, its default F4, F5 and F6 and their bandwidths (Table XII), and its KLGLOTT88 voicing source: open quotient, spectral tilt, flutter, double pulsing, open-phase F1 changes (supported by the synthesizer but not yet set by rules), and breathy phonation toward voiceless consonants and silence. |
| [Hertz, Kadin & Karplus (1985), *The Delta rule development system for speech synthesis from text*](https://users.soe.ucsc.edu/~karplus/papers/delta-85.pdf) | Representing speech as coordinated streams, and splitting words into roots and suffixes before letter-to-sound conversion. |
| Hertz (1991), *Streams, phones and transitions: toward a new phonological and phonetic model of formant timing*, Journal of Phonetics 19, 91-109 | The phone-and-transition timing model: formant transitions as timed units of their own between phones, with durations that depend on the two phones (stop, fricative, vowel, liquid and offglide joins); transitions that stay stable when syllables are lengthened, so only steady states stretch; aspiration spanning the release transition; and, in fast speech, transitions that keep their durations while steady states shrink, and a diphthong's interior transition that keeps its slope and is cut short (Figs. 16-17). |
| [Hertz & Huffman (1992), *A nucleus-based timing model applied to multi-dialect speech synthesis by rule*](https://www.isca-archive.org/icslp_1992/hertz92_icslp.pdf) | Timing a vowel and its following liquids and glides as one nucleus, which the vowel's context lengthens or shortens as a whole (so a coda /l/ is longer before a voiced consonant, as in Hertz 1991) and in which the vowel takes whatever time the transitions and other phones leave, and shortening the voiced vowel after aspiration. |
| [Hertz, Younes & Zinovieva (1999), *Language-universal and language-specific components in the multi-language ETI-Eloquence text-to-speech system*](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf) | Phones joined by explicit transitions, example transition timings, voicing and aspiration aligned with phone and transition edges, diphthongs as a vowel and a glide, and accent peaks a set percentage of the way through the nucleus (the paper gives no figure; its Fig. 5 example suggests about three quarters). This is the published description of Eloquence's own design. |
| Fry (1955), *Duration and intensity as physical correlates of linguistic stress*, JASA 27(4), 765-768; Lehiste (1970), *Suprasegmentals* | Greater intensity as a correlate of stress. |
| Löfqvist & McGowan (1992), *Influence of consonantal environment on voice source aerodynamics*, Journal of Phonetics 20 | Breathy voicing at vowel onsets after voiceless consonants that changes gradually into modal voicing, so voicing grows after aspiration rather than switching on. |
| Ladefoged & Johnson (2011), *A Course in Phonetics*, 6th ed. | American English connected speech: weak forms of function words, and flapping of /t/ and /d/ between a vowel and an unstressed vowel. |
| Zue & Laferriere (1979), *Acoustic study of medial /t,d/ in American English*, JASA 66(4), 1039-1050 | Flaps as brief voiced taps without a release burst. |
| Ladefoged & Maddieson (1996), *The Sounds of the World's Languages* | Taps and flaps as a single ballistic movement of the tongue tip. |
| Flemming & Johnson (2007), *Rosa's roses: reduced vowels in American English*, JIPA 37(1), 83-96 | Two reduced vowels: a high central [ɨ] between consonants and [ə] elsewhere. |
| Sussman, McCaffrey & Matthews (1991), *An investigation of locus equations as a source of relational invariance for stop place categorization*, JASA 90(3), 1309-1325 | The velar F2 locus equation (Table I, male talkers). |
| Lehiste & Peterson (1959), *Vowel amplitude and phonemic stress in American English*, JASA 31(4), 428-435 | Intrinsic vowel intensity: open vowels are louder than close ones, which a cascade with one voicing level per vowel reproduces. |
| Hawks & Miller (1995), *A formant bandwidth estimation procedure for vowel synthesis*, JASA 97(2), 1343-1344 | Vowel formant bandwidths as a function of formant frequency, fitted to natural vowels and scaled for F0 (coefficients as reproduced by VoiceSauce, Shue et al. 2011); diphthong bandwidths follow their gliding formants. |
| Fant (1960), *Acoustic Theory of Speech Production* | Resonances of a uniform male vocal tract every 1 kHz from 500 Hz, for the one fixed pole above KLSYN88's F6 (6.5 kHz) at output rates above 10 kHz. |

Modern neural and hybrid formant research ([KLATTSTAT, 2010](https://www.isca-archive.org/ssw_2010/anumanchipalli10b_ssw.pdf); [speaker-independent neural formant synthesis, 2023](https://arxiv.org/abs/2306.01957); [HiFi-Glot](https://arxiv.org/abs/2409.14823); [ARIS](https://arxiv.org/abs/2609.29923)) was read as background. It was not adopted: learned models trade determinism, latency and behaviour at extreme speaking rates for naturalness, and screen-reader users depend on the former.

### Where Klatter departs from the papers

Some values are not in the papers. They were chosen by listening or by neutral measurement, and are documented in the code:

- **Even rhythm and predictable delivery.** Stressed vowels are shorter and unstressed vowels in content words longer than Klatt's rules give (except in phrase-final syllables, which are already lengthened, and a schwa before a syllabic /l/, which the /l/ carries), the pitch range is narrower, and phrase-final lengthening is trimmed. Listeners preferred this evenly timed, less sing-song delivery to Klatt's rules alone. Vowels also differ less in inherent duration than in Klatt's (1979) table (AA, AO, AY and UH shorter, IY, IH and EY longer), and unstressed vowels in a word's last syllable get less extra length than earlier ones; with Klatt's own vowel values, or with one lengthening factor everywhere, speech recognition misheard more words of the Harvard sentences.
- **Shorter transitions.** Formant transitions take half the durations of Hertz's examples, which come from careful speech by one speaker. At full length they leave Klatter's short, evenly timed vowels too little steady state and dull the voice by about 1 dB at 1-4 kHz. Where a span is too short even for these, the transitions shorten so they still reach their targets; only diphthong glides are cut short as Hertz shows.
- **Separate pitch accents.** Each accent rises from and returns to a declining baseline, rather than pitch staying raised from the first accent to the last as in Klatt's (1987) hat pattern; holding a hat between accents made speech recognition mishear more words of the Harvard sentences.
- **Uniform fast speech.** In fast speech, transitions shrink in proportion with the phones, so every part of the speech shrinks by the same proportion, which listeners preferred. Hertz's fast speech keeps transitions at their natural durations (Figs. 16-17), but at screen-reader rates that leaves vowels little steady state and dulls the voice.
- **Brighter voicing and crisper consonants.** A 5 dB presence boost on voicing only, a 6 dB boost to frication and bursts, and stronger aspiration were preferred in blind listening.
- **Consonant voicing cues.** Fricative and burst levels were chosen by speech recognition of the Harvard sentences. As a result /z/ and /ʒ/ have frication only about 5 dB weaker than /s/ and /ʃ/, and /v/ and /ð/ stronger than /f/ and /θ/, where Klatt (1980) makes voiced frication 10 dB weaker with the same spectrum. Stop closures all carry a low voice bar so phrases have no silent gaps, although Klatt (1980) reserves the voice bar for voiced plosives and Hertz (1991) ends voicing before a voiceless closure. A trial of Klatt's values for both, together with the two departures below, made automatic speech recognition of Harvard sentences at 450 WPM worse, so the current values stay.
- **Smooth joins.** Voicing and frication cross-fade over about 25 ms, because switching them within one 5 ms frame sounds spliced; /h/ is voiced with noise on top, because a noise-only /h/ sounded strange.

Klatter's voice is male, so its voice-quality and stress-intensity rules follow Klatt & Klatt's (1990) measurements of male talkers, which in places differ from their summary across all talkers:

- **Modal unstressed syllables.** Klatt & Klatt (1990) summarize breathiness as greater in unstressed syllables, but their male talkers' first harmonic at mid-vowel was no stronger in unstressed syllables (Table II) and their vowels' noise ratings were low, if somewhat higher in unstressed syllables (Tables V and VI). Klatter keeps unstressed syllables modal and, as Klatt (1987) describes at utterance ends, reserves breathiness for phrase-final syllables, offsets into silence and the margins of voiceless consonants.
- **Breathy phrase-final syllables.** Phrase-final syllables are slightly breathy throughout (a higher open quotient and tilt as well as noise), although Klatt & Klatt's male talkers' first harmonic was 2.5 dB weaker in the last syllable (Table II), which they read as slight laryngealization; a noise-only final syllable was part of the trial above.
- **Shelved noise.** Klatt's (1980) noise low-pass is an integrator that the radiation characteristic exactly cancels, leaving flat noise. Klatter's low-pass leaves a high-pass shelf (about 8 dB down at 300 Hz relative to 4 kHz), with which the consonant and aspiration levels were set; flat noise was also part of that trial.
- **Short voiceless margins.** The breathy margins beside voiceless consonants follow the 80 ms ramps of Klatt & Klatt's Figs. 15 and 16 but stay within the outer third of a sonorant, because their Table II shows that even [h] does not make the middle of the vowel breathier.
- **Weaker unstressed vowels.** Stressed syllables are more intense (Fry 1955; Lehiste 1970), by 4.3 dB for Klatt & Klatt's male talkers, partly through their higher F0. Klatt (1987) relied on F0 for stress-related intensity and found that a specific rule to raise stressed vowels' intensity made them sound artificially strong. Klatter instead voices unstressed vowels 3 dB more weakly, which a listener preferred in blind comparison; with the pitch contour, stressed vowels are about 3.5 dB more intense than the same vowel unstressed.

### Pipeline

1. **Text** (`text.rs`) expands numbers, abbreviations, acronyms and identifiers, folds accents, and splits clauses at punctuation, keeping each word's original UTF-8 position.
2. **Pronunciation** (`lexicon.rs`, `morph.rs`, `lts.rs`) tries caller overrides, a small built-in table, then CMUdict. Words spelled -day end in a full [deɪ] ("Tuesday", "Saturday"), the first reading in American dictionaries such as Merriam-Webster, where CMUdict lists the reduced [di] first for some weekday names. Unknown words go through unstressed-prefix analysis (un-, re-, dis- and mis- before a dictionary word, as in "untitled"), suffix analysis, conservative compound splitting, and finally letter-to-sound rules with stress heuristics.
3. **Structure** (`utterance.rs`) syllabifies words, marks function words and phrase-final syllables, and applies American connected-speech forms. Function words take weak forms ("to" [tə], "for" [fɚ]) when another word follows in the phrase, and keep their strong forms when spoken alone or at the end of a phrase ("what is it for?"). Unstressed vowels reduce to [ɨ] ("rabbit", "roses"), except in "-ing", and to [ə] next to a vowel, at a word's edges and before /l/ ("about", "little"); a word-final [ə] keeps its quality before a plural or possessive ending ("Rosa's", "sofas"), as Flemming & Johnson (2007) describe. /t/ and /d/ flap after a vowel or /r/, before an unstressed vowel in the word ("water") or any vowel across a word boundary in the phrase ("get up"). Caller overrides keep their vowels as written; flapping applies to them too. A listener heard Klatter without these forms as Scottish- or Irish-accented.
4. **Prosody** (`prosody.rs`, `timing.rs`) applies the duration rules, nucleus timing and aspiration. Phones and formant transitions are then timed as separate units: each transition's duration depends on the two phones it joins, a phone's steady state is what its segment leaves after the transitions inside it, and within a syllable nucleus the vowel gives up the time. Prosody then builds the pitch contour: a declining baseline with a rise-fall pitch accent on each stressed content word, accent peaks about three quarters of the way through the nucleus, boundary tones for statements, questions and commas, and consonant microprosody. Pauses are 165 ms at commas and 385 ms at sentence ends at the natural rate, and shrink faster than speech as the rate rises.
5. **Acoustics** (`params.rs`) turns phonemes into 5 ms parameter frames: formant targets held through each phone's steady state and interpolated across the timed transitions, stop closures with a low voice bar, flaps as brief voiced taps, bursts, aspiration, frication, nasalization, and voicing and noise cross-fades so sounds are not spliced together.
6. **Synthesis** (`klatt.rs`) is KLSYN88. The voicing source runs at four times the output rate, so pitch periods need not be whole samples, and formant frequencies glide sample by sample between frames so changing a ringing filter never clicks.
7. **Voice settings** (`lib.rs`) scale durations for rate and reshape the pitch contour for pitch and inflection.

### How changes were evaluated

Speech quality cannot be unit-tested, so every audible change was judged by listening, using tools in `tools/`:

- **Blind A/B tests, one pair at a time.** Both versions are calibrated to the same measured speaking rate and loudness, presented in random order, with the question spoken in the audio. Changes were kept only when preferred, or when they measurably helped without sounding worse.
- **Intelligibility and acoustic checks.** Speech recognition (Whisper) on Harvard sentences and minimal-pair carrier words, at the natural rate and at 450 words per minute, compared each change with the previous version; per-frame parameter dumps were checked against the papers' rules; and brightness, clipping and speaking-rate accuracy were measured on long readings.
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
- Phones and transitions are timed separately, but Klatt's segment durations still set the time they share, rather than Hertz's own duration rules. Hertz's further streams (separate timing per acoustic parameter, voicing that stops partway through a transition before a voiceless consonant) are not modelled.

## Licensing

Klatter is licensed under the terms of the MIT license.

The CMU Pronouncing Dictionary is distributed under its own BSD-style licence, in `data/cmudict/LICENSE.txt`, which must accompany binary distributions (the NVDA builder includes it).
