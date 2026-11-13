# Klatter - A modern implementation of Dennis Klatt's 1980 TTS engine

This is a modern TTS engine written in Rust, based on Dennis Klatt's 1980's research.

The core engine is based on the [original 1980 paper](https://www.fon.hum.uva.nl/david/ma_ssp/2009/klatt-1980-JAS000971.pdf) and the [broader 1987 architectural paper](https://sail.usc.edu/~lgoldste/Ling582/Week%2013/Klatt1987.pdf). The paper is based on these papers, without reusing existing source code.

It was then improved based on [this 1990 paper](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1990-JAS000820.pdf).

It was then improved using Susan Hertz's papers, whose work also underlies the ETI-Eloquence engine:

- [English text to speech rules with Delta (1986)](https://www.academia.edu/56835165/English_text_to_speech_conversion_with_delta): splitting words into roots and suffixes before letter-to-sound conversion.
- [A nucleus-based timing model applied to multi-dialect speech synthesis by rule (1992)](https://www.isca-archive.org/icslp_1992/hertz92_icslp.pdf): timing the vowel and its following liquids and glides as one nucleus, and shortening the voiced vowel after aspiration.
- [Language-universal and language-specific components in the multi-language ETI-Eloquence text-to-speech system (1999)](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf): transition timings, the timing inside AY, AW and OY, and where the accent peak falls.

In addition to the engine, there is a command-line tool which accepts input and output paths, and a speaking rate.

## Building and running

```
cargo build --release
target\release\klatter in.txt out.wav
target\release\klatter -r 300 in.txt out.wav
```

The rate is in words per minute, as with `say -r` (`--rate=300` also works); the default is Klatter's natural rate of 220. Every segment and pause scales by the same factor. Breathiness ramps scale with the rate so they do not spread across entire vowels at fast rates. Other acoustic transition timings retain their nominal durations, constrained by the available segment frames.

The output is a 16 kHz, 16-bit mono WAV file at a fixed gain, so short and long utterances keep their natural relative loudness. Repeated calls to `klatt::Synth::render` on the same synthesizer preserve both synthesis state and fractional-sample timing, so rendering frames in chunks produces the same samples as rendering them together. The project uses no third-party crates.

CMUdict is bundled with the engine. Redistributions must include `data/cmudict/LICENSE.txt`; the NVDA builder includes it automatically.

### Library

`klatter::synthesize(text, sample_rate)` returns samples at any sample rate. For hosts such as screen readers, `klatter::speak(items, sample_rate, voice)` takes a `Voice` and a list of `Item`s: text interleaved with numbered marks. It speaks the items as one utterance, so a mark does not break the phrasing, and returns the samples together with the sample index each mark falls on. `Voice` is in physical units: words per minute, a pitch baseline in Hz (93 by default; the whole contour moves with it), an inflection multiplier that stretches the contour around that baseline (0 is monotone), and a linear volume. Hosts map their own scales onto these.

Relevant sources for further general rules:

- [Hertz, Younes and Zinovieva (1999), section 3](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf): explicit phones and transitions, nucleus duration allocation, and context-dependent values at phone edges.
- [eSpeak NG phoneme tables](https://github.com/espeak-ng/espeak-ng/blob/master/docs/phontab.md): context-sensitive phoneme variants, consonant effects on vowel length, and separately specified vowel entry/exit transitions. Its [length calculation](https://github.com/espeak-ng/espeak-ng/blob/master/src/libespeak-ng/setlengths.c) also considers following consonants, stress and syllable position. No code or tables are imported.
- [Festival duration module](https://github.com/festvox/festival/blob/master/lib/duration.scm): both hand-written Klatt rules and CART-based duration models, including phone-normalized log-duration predictions. Klatter does not use a learned duration model.
- [CMUdict](https://github.com/cmusphinx/cmudict): the bundled English pronunciation dictionary. It is not a duration or acoustic-trajectory model, and its maintainers explicitly caution that entries can contain errors.

### C interface

The `ffi` crate builds `klatter.dll` with the C functions declared in `ffi/klatter.h`. `klatter_speak` takes parallel arrays of UTF-8 strings and mark ids (a null string means "mark here") plus a `KlatterVoice`, and returns 16-bit samples and mark positions that the caller releases with `klatter_free`. It returns null for invalid input and never unwinds a panic into the caller.

### NVDA add-on

`nvda/addon` holds an NVDA synthesizer driver that loads `klatter.dll` with ctypes. It supports rate, pitch, inflection and volume, index marks and mid-utterance pitch changes; NVDA's 0-100 settings map onto `KlatterVoice`, with 50 giving Klatter's natural voice and each 50 steps doubling or halving the rate or pitch. NVDA is a 64-bit x86 program, so the add-on carries an x64 DLL, which runs under emulation on ARM64 Windows.

```
rustup target add x86_64-pc-windows-msvc
python nvda\build.py
```

This writes `target\klatter-<version>.nvda-addon`; open it to install it in NVDA, then restart NVDA and choose Klatter in the Speech settings. `python nvda\test_driver.py target\release\klatter.dll` tests the driver against stand-ins for NVDA's modules; the DLL must match the Python interpreter's architecture.

## Pipeline

1. `text.rs` normalizes the text. It expands numbers, years, ordinals, money, percentages and common abbreviations, spells out acronyms, and splits clauses at punctuation. Typographic apostrophes are normalized before word splitting. Negative numbers retain their sign, and numbers with thousands separators are read as quantities rather than years.
2. `lexicon.rs` tries hand-written pronunciation overrides, then exact matches in CMUdict. Unmatched words go through `morph.rs`, which strips inflectional suffixes, looks up the root, and adds a suffix pronunciation matching the root's last sound. Remaining words use `lts.rs` letter-to-sound rules and stress heuristics.
3. `utterance.rs` syllabifies each word and marks function words and phrase-final syllables.
4. `prosody.rs` applies Klatt's segment duration rules, retuned toward an even, syllable-timed rhythm: stressed vowels, stop closures and fricatives are shorter than Klatt's values, and unstressed vowels in content words are longer. Pauses are short (100 ms at commas, 200 ms at sentence ends); boundaries are marked mostly by pitch, and phrase-final vowels are not lengthened beyond escaping non-final shortening, which avoids drawled sentence ends. A vowel and the liquids and glides after it in the same syllable are timed as one nucleus, so a longer coda leaves less time for the vowel. Aspiration after a voiceless stop lengthens the syllable while shortening the following sonorant's voiced portion, subject to its minimum duration. The pitch contour is a hat pattern with declination and microprosody: a voiceless consonant raises the onset of the following sonorant by 8% and a voiced one lowers it by 4%, decaying over 50 ms. Each accent peaks about three quarters of the way through its nucleus. Clauses without pitch accents still receive a terminal contour, starting at their last vowel.
5. `params.rs` turns the phonemes into 5 ms parameter frames using synthesis-by-rule targets and transitions. Formant transitions request about 15 ms after fricatives and 35 ms after aspirated stops, and close quickly after an offglide. That is half the published rule timings, so that short vowels reach their targets instead of drifting toward their neighbours; vowel targets are Klatt's (1980) citation-form values, with EY and OW as true diphthongs, because targets measured from running speech are already undershot and the transitions would undershoot them again, merging vowels such as EH and IH. Nasal formants are tuned separately. Vowels are nasalized over their last and first 30 ms next to a nasal, and affricates spend about two thirds of their time as frication. Every stop closure, voiced or voiceless, carries a low voice-bar murmur, so speech doesn't break into silence mid-phrase. Voicing and fricative noise cross-fade over about 25 ms at segment boundaries so sounds don't appear spliced together; stop bursts stay abrupt. Vowel first-formant bandwidths are wider than Klatt's, so the F1 peak doesn't drown out the rest of the spectrum. Vowel voicing levels vary so vowels are evenly loud, nasals are slightly louder than liquids and glides, and /h/ is voiced with noise on top rather than pure noise. Incoming and outgoing windows share the segment's available time without overlapping, preserving aspiration and a target frame. AY, AW and OY use an 80 ms glide and then hold the offglide for 50 ms when the available duration permits, with time reserved for the outgoing transition; short diphthongs scale proportionally. These durations are modeling choices informed by the papers, not a complete implementation of Hertz's phone-and-transition model. Following the 1990 paper, vowels become breathier toward an offset into silence, next to voiceless consonants, and in unstressed or phrase-final syllables. Breathier voicing means a higher open quotient, more spectral tilt, aspiration noise and wider first and second formant bandwidths. Voicing also weakens by about 10 dB as it fades into silence.
6. `klatt.rs` contains KLSYN88, the cascade/parallel formant synthesizer as described in the 1990 paper. The KLGLOTT88 voicing source runs at four times the output rate so pitch periods can be a fraction of a sample long. It has open quotient, a spectral tilt filter, flutter and diplophonia controls. The first formant can change while the glottis is open, and a tracheal pole-zero pair can be added. Above F3 the cascade uses fixed resonances at 3.4, 3.9, 4.7 and 6 kHz so that vowels keep their energy at 3.5-5 kHz. Only voicing passes these; aspiration noise keeps F4/F5 at 3.25 and 3.7 kHz so breath doesn't hiss, and the parallel frication branch keeps its own F4-F6.
7. `lib.rs` applies the voice settings: it scales durations for the rate and reshapes the F0 contour for pitch and inflection before synthesis, then applies the gain and volume.
8. `wav.rs` writes the WAV file.

Pauses do not restart voicing, and retain the preceding sound's formants, nasalization and tilt while it decays, until the next onset needs its own settings. The synthesizer interpolates cascade formant frequencies and bandwidths sample by sample between parameter frames, including across rendering calls, so changing a ringing filter does not introduce a sudden coefficient step.

## Regression coverage

Regression tests cover consonant-cluster shortening, terminal intonation in unaccented clauses, aspiration and offglide timing, typographic apostrophes, grouped and negative numbers, plural roots and stress preservation, the AW vowel in DOWN and its inflections, and incremental rendering at fractional samples per frame. Rendering chunk sizes are checked for exact waveform equivalence. These checks establish correctness of the specified behaviors, not perceived voice quality.

Rate-specific checks cover the relative timing of breathy offsets and preservation of a less breathy vowel core at faster rates. Pause tests check that voicing does not leak into pauses and that ringing tract settings are retained. Source tests cover abrupt formant-change transients and check that open-glottis modulation follows the decimation filter's delay, keeping noise modulation and open-phase formant changes aligned with the glottal flow at different pitches and sample rates.

## Listening and pronunciation diagnostics

```
python tools\listen.py passage.txt baseline.exe candidate.exe --out work\listening --wpm 300
cargo run --release --example phones < passage.txt
cargo run --release --example lts_accuracy
```

The listening tool measures actual rate, matches active-speech RMS with shared
clipping headroom, and presents one blind pair per paragraph. Its question is
spoken in the audio. Order is balanced across trials, replay uses the same WAV,
and `manifest.json` records the seed, text, engine settings, executable hashes,
levels, actual rates and votes. Keep the manifest hidden until voting.
`--render-only` prepares clips without playback; an existing output directory
is never overwritten. Calibration trims edge silence but retains internal pauses.

The default two-executable comparison needs only Python's standard library.
The adapters also support installed SAPI voices and eSpeak (`espeakng-loader`
required). Comparing different
sample rates requires SciPy; clips are resampled to the highest native rate.
External engines and recognizer models are not bundled or downloaded.

`phones` reports intended pronunciations after normalization. `lts_accuracy`
measures raw letter-to-sound agreement against first CMUdict readings, without
morphology or stress scoring. This is a diagnostic, not a listening-quality
score or an estimate of real unknown-word accuracy.

## Voice metrics

```
python tools\voice_metrics.py reference.wav candidate.wav
```

Compares a rendering against a reference recording of the same text using metrics that do not depend on speaking rate: F0 distribution, H1-H2, median F1-F3, pause frequency and length, and long-term spectrum bands relative to 250-500 Hz. Each difference is expressed in tolerances, and the score is their root mean square (0 means a match). Worst mismatches are listed first. Requires Python with numpy, and 16-bit WAV at 16 kHz or above.

## Known limitations

- Pronunciation does not use sentence context to distinguish words such as present- and past-tense READ. Words absent from both dictionaries depend on morphology and letter-to-sound guesses.
- There is no emphasis markup, and phrasing comes from punctuation only.
- Transitions still occupy phoneme durations rather than independently timed acoustic units. Lexical stress and phrase prominence are not modeled separately.
- The input must be English.
