# Klatter - A modern implementation of Dennis Klatt's 1980 TTS engine

This is a modern TTS engine written in Rust, based on Dennis Klatt's 1980's research.

The core engine is based on the [original 1980 paper](https://www.fon.hum.uva.nl/david/ma_ssp/2009/klatt-1980-JAS000971.pdf) and the [broader 1987 architectural paper](https://sail.usc.edu/~lgoldste/Ling582/Week%2013/Klatt1987.pdf). The paper is based on these papers, without reusing existing source code.

It was then improved based on [this 1990 paper](https://www.fon.hum.uva.nl/david/ma_ssp/doc/Klatt-1990-JAS000820.pdf).

It was then improved using Susan Hertz's papers, whose work also underlies the ETI-Eloquence engine:

- [English text to speech rules with Delta (1986)](https://www.academia.edu/56835165/English_text_to_speech_conversion_with_delta): splitting words into roots and suffixes before letter-to-sound conversion.
- [A nucleus-based timing model applied to multi-dialect speech synthesis by rule (1992)](https://www.isca-archive.org/icslp_1992/hertz92_icslp.pdf): timing the vowel and its following liquids and glides as one nucleus, and shortening the voiced vowel after aspiration.
- [Language-universal and language-specific components in the multi-language ETI-Eloquence text-to-speech system (1999)](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf): transition timings, the timing inside AY, AW and OY, and where the accent peak falls.

In addition to the engine, there is a command-line tool which accepts input and output paths, with an optional vowel timing setting.

## Building and running

```
cargo build --release
target\release\klatter in.txt out.wav
```

The output is a 16 kHz, 16-bit mono WAV file. The library function `klatter::synthesize(text, sample_rate)` also accepts other sample rates. Repeated calls to `klatt::Synth::render` on the same synthesizer preserve both synthesis state and fractional-sample timing, so rendering frames in chunks produces the same samples as rendering them together. The project uses no third-party crates.

### Voice profiles

```
target\release\klatter --voice light --timing compact-holds in.txt out.wav
```

`--voice original` is the default. The opt-in `light` voice uses a shorter-tract approximation: tract resonance frequencies are multiplied by 1.22 and bandwidths by 1.30. This includes F1-F5 in the cascade, parallel frication resonances through F6, open-glottis F1 adjustments, and nasal/tracheal pole-zero filters. Broader bandwidths reduce resonance ringing; scaling both branches keeps vowels and consonants in the same voice. The existing sample-rate-dependent resonance bypass still applies near Nyquist.

Pitch periods, glottal shape, spectral tilt, source amplitudes and noise generation are unchanged. This is a tract-profile change, not pitch shifting, resampling or added breathiness. It is intended to produce a distinctly lighter resonance pattern; perceived quality still requires listening. Pronunciation, timing and contextual-trajectory options remain independent, with original trajectories selected by default.

Use `klatt::Synth::with_voice(sample_rate, klatt::Voice::Light)` to render the same frames with the new profile, or `synthesize_with_voice(text, sample_rate, timing, trajectory, voice)` for normalized PCM. Existing library entry points retain the original voice. CLI output is peak-normalized; controlled voice comparisons should match playback level separately.

### Compact vowel holds

```
target\release\klatter --timing compact-holds in.txt out.wav
```

`--timing original` is the default. Compact timing finds each vowel's longest steady formant interval after aspiration. It retains the first 20 ms of dwell plus 25% of any excess, subject to the existing stress-sensitive vowel minimum plus aspiration. Additional frames are retained where necessary to avoid creating F0 steps above 3 Hz per 5 ms (or increasing an already steeper contour). These shared coefficients are modeling choices, not published rules. The timing policy does not depend on word names or change pronunciation.

All moving formant transitions, consonants, pauses and aspiration frames are copied unchanged. Within a shortened hold, the existing frame parameters are linearly resampled, preserving both endpoints rather than splicing across a pitch or breathiness change. F0 is time-mapped, not regenerated or shifted; its time-weighted mean can nevertheless change. This does not shorten slow diphthong glides or fix incorrect phoneme choices, and it is not a uniform speaking-rate adjustment.

Use `synthesize_frames_with_timing(text, params::Timing::CompactHolds)` for raw frames, or `synthesize_with_timing(text, sample_rate, params::Timing::CompactHolds)` for normalized PCM. `params::build_frames_with_timing` also accepts already-timed segments. Compaction occurs after the original frame layout is built; segment durations and `params::frame_layout` still describe the original timeline, not offsets into compacted frames.

Relevant sources for further general rules:

- [Hertz, Younes and Zinovieva (1999), section 3](https://www.synfonicaspeech.com/pdf/ICPhS99.pdf): explicit phones and transitions, nucleus duration allocation, and context-dependent values at phone edges. Compact timing borrows the hold/transition distinction, not their complete acoustic model.
- [eSpeak NG phoneme tables](https://github.com/espeak-ng/espeak-ng/blob/master/docs/phontab.md): context-sensitive phoneme variants, consonant effects on vowel length, and separately specified vowel entry/exit transitions. Its [length calculation](https://github.com/espeak-ng/espeak-ng/blob/master/src/libespeak-ng/setlengths.c) also considers following consonants, stress and syllable position. No code or tables are imported.
- [Festival duration module](https://github.com/festvox/festival/blob/master/lib/duration.scm): both hand-written Klatt rules and CART-based duration models, including phone-normalized log-duration predictions. Klatter does not use a learned duration model.
- [CMUdict](https://github.com/cmusphinx/cmudict): a broad English pronunciation reference for evaluating phoneme sequences and stress. It is not a duration or acoustic-trajectory model, and its maintainers explicitly caution that entries can contain errors. It is not bundled here.

### Experimental contextual vowel trajectories

```
target\release\klatter --timing compact-holds --trajectory contextual in.txt out.wav
```

`--trajectory original` is the default. Contextual trajectories change only F1-F3 inside voiced monophthongs. A two-sided exponential target-approach curve uses the vowel's existing target and its first/last voiced formant values, which already reflect adjacent sounds. A shared 40 ms time constant lets longer vowels approach their targets more closely than shorter vowels. The curve is blended 65% with the original trajectory to limit undershoot. These are experimental modeling choices, not published values or a demonstrated quality improvement.

The first and last voiced frames, aspiration, consonants, diphthong trajectories, formant bandwidths and all source/pitch controls remain unchanged. Timing compaction is completed before the trajectory change, so adding formant movement cannot remove the accepted shortening or change the frame count. Neighbour influence follows vowel duration and acoustic boundary values, not word names; there is no random movement. When both boundaries already equal the vowel target, the model introduces no movement.

The experiment draws on the target-undershoot hypothesis associated with [Lindblom's study of vowel reduction](https://doi.org/10.1121/1.1918816), and the phone-edge/context distinction in Hertz's model. It is not a reproduction of either model; vowel undershoot is not a universal requirement, and intelligibility must be assessed by listening.

Use `synthesize_frames_with_options(text, timing, params::Trajectory::Contextual)` or `synthesize_with_options(text, sample_rate, timing, params::Trajectory::Contextual)`. Existing timing-only APIs retain the original trajectories. The options are independent; both timing and trajectory must be selected to use compact holds with contextual movement.

## Pipeline

1. `text.rs` normalizes the text. It expands numbers, years, ordinals, money, percentages and common abbreviations, spells out acronyms, and splits clauses at punctuation. Typographic apostrophes are normalized before word splitting. Negative numbers retain their sign, and numbers with thousands separators are read as quantities rather than years.
2. `lexicon.rs` holds a dictionary of exceptions and roots needed to disambiguate inflections. Words not in it are tried by `morph.rs`, which strips the suffixes -s, -ed, -ing and -ly, repairs the root's spelling (for example hoped to hope), and adds a suffix pronunciation that matches the root's last sound. Ambiguous -ses and -zes endings prefer a unique dictionary-backed lexical root. If neither candidate is known, the final -s is stripped so root-level pronunciation and stress rules still apply; if both are known, the word stays whole. Everything else goes to `lts.rs`, which applies letter-to-sound rules and stress heuristics.
3. `utterance.rs` syllabifies each word and marks function words and phrase-final syllables.
4. `prosody.rs` applies Klatt's segment duration rules. A vowel and the liquids and glides after it in the same syllable are timed as one nucleus, so a longer coda leaves less time for the vowel. Aspiration after a voiceless stop lengthens the syllable while shortening the following sonorant's voiced portion, subject to its minimum duration. The pitch contour is a hat pattern with declination, and each accent peaks about three quarters of the way through its nucleus. Clauses without pitch accents still receive a terminal contour, starting at their last vowel.
5. `params.rs` turns the phonemes into 5 ms parameter frames using synthesis-by-rule targets and transitions. Formant transitions request about 30 ms after fricatives and 70 ms after aspirated stops, and close quickly after an offglide. Incoming and outgoing windows share the segment's available time without overlapping, preserving aspiration and a target frame. AY, AW and OY use an 80 ms glide and a brief offglide when the available duration permits, with time reserved for the outgoing transition; short diphthongs scale proportionally. These durations are modeling choices informed by the papers, not a complete implementation of Hertz's phone-and-transition model. Following the 1990 paper, vowels become breathier toward an offset into silence, next to voiceless consonants, and in unstressed or phrase-final syllables. Breathier voicing means a higher open quotient, more spectral tilt, aspiration noise and wider first and second formant bandwidths. Voicing also weakens by about 10 dB as it fades into silence.
6. `klatt.rs` contains KLSYN88, the cascade/parallel formant synthesizer as described in the 1990 paper. The KLGLOTT88 voicing source runs at four times the output rate so pitch periods can be a fraction of a sample long. It has open quotient, a spectral tilt filter, flutter and diplophonia controls. The first formant can change while the glottis is open, and a tracheal pole-zero pair can be added.
7. `wav.rs` writes the WAV file.

## Regression coverage

Regression tests cover consonant-cluster shortening, terminal intonation in unaccented clauses, aspiration and offglide timing, typographic apostrophes, grouped and negative numbers, plural roots and stress preservation, the AW vowel in DOWN and its inflections, and incremental rendering at fractional samples per frame. Rendering chunk sizes are checked for exact waveform equivalence. These checks establish correctness of the specified behaviors, not perceived voice quality.

## Voice metrics

```
python tools\voice_metrics.py reference.wav candidate.wav
```

Compares a rendering against a reference recording of the same text using metrics that do not depend on speaking rate: F0 distribution, H1-H2, median F1-F3, pause frequency and length, and long-term spectrum bands relative to 250-500 Hz. Each difference is expressed in tolerances, and the score is their root mean square (0 means a match). Worst mismatches are listed first. Requires Python with numpy, and 16-bit WAV at 16 kHz or above.

## Known limitations

- Letter-to-sound rules guess wrong on some irregular words. You can add those words to the dictionary in `lexicon.rs`.
- There is no emphasis markup, and phrasing comes from punctuation only.
- Transitions still occupy phoneme durations rather than independently timed acoustic units. Lexical stress and phrase prominence are not modeled separately.
- The input must be English.
