//! C interface to the engine, declared in `klatter.h`, for hosts that are not
//! written in Rust.

use std::ffi::{CStr, c_char, c_int, c_void};
use std::panic::{AssertUnwindSafe, catch_unwind};

use engine::{Item, Voice};

#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct KlatterVoice {
    pub wpm: f64,
    pub pitch_hz: f64,
    pub inflection: f64,
    pub volume: f64,
}

#[repr(C)]
pub struct KlatterSpeech {
    pub samples: *const i16,
    pub sample_count: usize,
    pub mark_ids: *const u32,
    pub mark_samples: *const usize,
    pub mark_count: usize,
}

/// The header comes first so a pointer to this is also a pointer to it.
#[repr(C)]
struct OwnedSpeech {
    header: KlatterSpeech,
    samples: Vec<i16>,
    mark_ids: Vec<u32>,
    mark_samples: Vec<usize>,
}

/// # Safety
/// `out` must point to writable memory for one `KlatterVoice`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_default_voice(out: *mut KlatterVoice) {
    let v = Voice::default();
    if let Some(out) = unsafe { out.as_mut() } {
        *out = KlatterVoice {
            wpm: v.wpm,
            pitch_hz: v.pitch_hz,
            inflection: v.inflection,
            volume: v.volume,
        };
    }
}

/// Speaks `count` items as one utterance. Item `i` is the UTF-8 text
/// `texts[i]`, or the mark `marks[i]` when `texts[i]` is null. Returns null for
/// invalid input; otherwise the result must be released with `klatter_free`.
///
/// # Safety
/// `texts` and `marks` must each point to `count` elements (`marks` may be
/// null if no item is a mark), every non-null text must be NUL-terminated, and
/// `voice` must point to a `KlatterVoice`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_speak_quality(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
    quality: *const f64,
    quality_count: usize,
) -> *mut KlatterSpeech {
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        with_input(texts, marks, count, sample_rate, voice, quality, quality_count, |items, voice| {
            own_speech(engine::speak(items, sample_rate, voice))
        })
    }));
    result.ok().flatten().unwrap_or(std::ptr::null_mut())
}

unsafe fn with_input<R>(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
    quality: *const f64,
    quality_count: usize,
    speak: impl FnOnce(&[Item], &Voice) -> R,
) -> Option<R> {
    let v = unsafe { voice.as_ref() }?;
    let valid = v.wpm.is_finite()
        && v.wpm > 0.0
        && v.pitch_hz.is_finite()
        && v.pitch_hz > 0.0
        && v.inflection.is_finite()
        && v.inflection >= 0.0
        && v.volume.is_finite()
        && v.volume >= 0.0
        && sample_rate > 0
        && (count == 0 || !texts.is_null());
    if !valid {
        return None;
    }
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let text = unsafe { *texts.add(i) };
        if text.is_null() {
            if marks.is_null() {
                return None;
            }
            items.push(Item::Mark(unsafe { *marks.add(i) }));
        } else {
            let text = unsafe { CStr::from_ptr(text) }.to_str().ok()?;
            items.push(Item::Text(text));
        }
    }
    let mut voice = Voice {
        wpm: v.wpm,
        pitch_hz: v.pitch_hz,
        inflection: v.inflection,
        volume: v.volume,
        ..Voice::default()
    };
    if !quality.is_null() {
        let count = quality_count.min(engine::PARAMETERS.len());
        for i in 0..count {
            // NaN keeps the setting's default.
            voice.quality.set_index(i, unsafe { *quality.add(i) });
        }
    }
    Some(speak(&items, &voice))
}

fn own_speech(speech: engine::Speech) -> *mut KlatterSpeech {
    let (mark_ids, mark_samples) = speech.marks.into_iter().unzip();
    let mut owned = Box::new(OwnedSpeech {
        header: KlatterSpeech {
            samples: std::ptr::null(),
            sample_count: 0,
            mark_ids: std::ptr::null(),
            mark_samples: std::ptr::null(),
            mark_count: 0,
        },
        samples: speech.samples,
        mark_ids,
        mark_samples,
    });
    owned.header = KlatterSpeech {
        samples: owned.samples.as_ptr(),
        sample_count: owned.samples.len(),
        mark_ids: owned.mark_ids.as_ptr(),
        mark_samples: owned.mark_samples.as_ptr(),
        mark_count: owned.mark_ids.len(),
    };
    Box::into_raw(owned).cast()
}

pub type SpeechCallback = unsafe extern "C" fn(*const KlatterSpeech, *mut c_void) -> c_int;

/// Streams borrowed audio blocks; returns 1 on completion, 0 on cancellation,
/// or -1 on invalid input or synthesis failure. The callback returns zero to cancel.
///
/// # Safety
/// Inputs have the same requirements as [`klatter_speak`]. The callback must not
/// unwind, free the block, or retain its pointers after returning. `context`
/// must be valid for the callback. All calls are synchronous on the calling thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_speak_stream_quality(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
    quality: *const f64,
    quality_count: usize,
    callback: Option<SpeechCallback>,
    context: *mut c_void,
) -> c_int {
    let Some(callback) = callback else { return -1 };
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        with_input(texts, marks, count, sample_rate, voice, quality, quality_count, |items, voice| {
            engine::speak_stream(items, sample_rate, voice, |samples, marks| {
                let (ids, offsets): (Vec<_>, Vec<_>) = marks.iter().copied().unzip();
                let block = KlatterSpeech {
                    samples: samples.as_ptr(),
                    sample_count: samples.len(),
                    mark_ids: ids.as_ptr(),
                    mark_samples: offsets.as_ptr(),
                    mark_count: ids.len(),
                };
                callback(&block, context) != 0
            })
        })
    }));
    match result {
        Ok(Some(completed)) => c_int::from(completed),
        _ => -1,
    }
}

/// # Safety
/// `speech` must be null or a pointer returned by `klatter_speak` that has
/// not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_free(speech: *mut KlatterSpeech) {
    if !speech.is_null() {
        drop(unsafe { Box::from_raw(speech.cast::<OwnedSpeech>()) });
    }
}

pub struct KlatterPronunciations(engine::Pronunciations);

#[unsafe(no_mangle)]
pub extern "C" fn klatter_pronunciations_create() -> *mut KlatterPronunciations {
    catch_unwind(|| {
        Box::into_raw(Box::new(KlatterPronunciations(
            engine::Pronunciations::default(),
        )))
    })
    .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `dictionary` must be a live, exclusively accessed handle; strings must be NUL-terminated.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_pronunciations_set(
    dictionary: *mut KlatterPronunciations,
    word: *const c_char,
    phones: *const c_char,
) -> c_int {
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let dictionary = dictionary.as_mut()?;
        if word.is_null() || phones.is_null() {
            return None;
        }
        let word = CStr::from_ptr(word).to_str().ok()?;
        let phones = CStr::from_ptr(phones).to_str().ok()?;
        dictionary.0.insert(word, phones).ok()
    }));
    if matches!(result, Ok(Some(()))) {
        1
    } else {
        -1
    }
}

/// # Safety
/// `dictionary` must be null or a live, exclusively accessed handle, freed only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_pronunciations_free(dictionary: *mut KlatterPronunciations) {
    if !dictionary.is_null() {
        drop(unsafe { Box::from_raw(dictionary) });
    }
}

#[repr(C)]
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct KlatterWord {
    pub input_item: usize,
    pub byte_offset: usize,
    pub byte_length: usize,
    pub sample: usize,
}

#[repr(C)]
#[derive(Default)]
pub struct KlatterPullBlock {
    pub sample_count: usize,
    pub mark_ids: *const u32,
    pub mark_samples: *const usize,
    pub mark_count: usize,
    pub words: *const KlatterWord,
    pub word_count: usize,
}

pub struct KlatterUtterance {
    stream: engine::Utterance,
    mark_ids: Vec<u32>,
    mark_samples: Vec<usize>,
    words: Vec<KlatterWord>,
}

pub struct KlatterCancel(engine::CancelHandle);

/// # Safety
/// Input arrays follow [`klatter_speak`]. An optional dictionary must be live and
/// not concurrently modified. Inputs and overrides are copied before returning.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_utterance_create_quality(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
    quality: *const f64,
    quality_count: usize,
    dictionary: *const KlatterPronunciations,
) -> *mut KlatterUtterance {
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        with_input(texts, marks, count, sample_rate, voice, quality, quality_count, |items, voice| {
            let empty = engine::Pronunciations::default();
            let pronunciations = dictionary.as_ref().map_or(&empty, |d| &d.0);
            engine::Utterance::new(items, sample_rate, voice, pronunciations).map(|stream| {
                Box::new(KlatterUtterance {
                    stream,
                    mark_ids: Vec::new(),
                    mark_samples: Vec::new(),
                    words: Vec::new(),
                })
            })
        })
    }));
    match result {
        Ok(Some(Ok(utterance))) => Box::into_raw(utterance),
        _ => std::ptr::null_mut(),
    }
}

/// # Safety
/// `utterance` must be live and exclusively accessed; `output` must hold `capacity`
/// samples (null is allowed only at capacity zero). `block` must be writable.
/// Returned metadata is borrowed until the next read or utterance destruction.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_utterance_read(
    utterance: *mut KlatterUtterance,
    output: *mut i16,
    capacity: usize,
    block: *mut KlatterPullBlock,
) -> c_int {
    let Some(block) = (unsafe { block.as_mut() }) else {
        return -1;
    };
    *block = KlatterPullBlock::default();
    if utterance.is_null()
        || capacity > isize::MAX as usize / std::mem::size_of::<i16>()
        || (capacity > 0 && output.is_null())
    {
        return -1;
    }
    let result = catch_unwind(AssertUnwindSafe(|| unsafe {
        let utterance = &mut *utterance;
        let output = if capacity == 0 {
            &mut []
        } else {
            std::slice::from_raw_parts_mut(output, capacity)
        };
        let rendered = utterance.stream.render(output);
        (utterance.mark_ids, utterance.mark_samples) = rendered.marks.into_iter().unzip();
        utterance.words = rendered
            .words
            .into_iter()
            .map(|word| KlatterWord {
                input_item: word.item,
                byte_offset: word.text_range.start,
                byte_length: word.text_range.len(),
                sample: word.sample,
            })
            .collect();
        *block = KlatterPullBlock {
            sample_count: rendered.samples,
            mark_ids: utterance.mark_ids.as_ptr(),
            mark_samples: utterance.mark_samples.as_ptr(),
            mark_count: utterance.mark_ids.len(),
            words: utterance.words.as_ptr(),
            word_count: utterance.words.len(),
        };
        if utterance.stream.cancel_handle().is_cancelled() {
            -2
        } else if rendered.samples > 0
            || block.mark_count > 0
            || block.word_count > 0
            || !utterance.stream.is_finished()
        {
            1
        } else {
            0
        }
    }));
    match result {
        Ok(status) => status,
        Err(_) => {
            unsafe { (*utterance).stream.cancel_handle().cancel() };
            *block = KlatterPullBlock::default();
            -1
        }
    }
}

/// # Safety
/// `utterance` must be live and not concurrently read or destroyed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_utterance_cancel_handle(
    utterance: *const KlatterUtterance,
) -> *mut KlatterCancel {
    catch_unwind(AssertUnwindSafe(|| unsafe {
        utterance.as_ref().map_or(std::ptr::null_mut(), |u| {
            Box::into_raw(Box::new(KlatterCancel(u.stream.cancel_handle())))
        })
    }))
    .unwrap_or(std::ptr::null_mut())
}

/// # Safety
/// `cancel` must be null or a live cancel handle. May run on another thread.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_cancel(cancel: *const KlatterCancel) {
    if let Some(cancel) = unsafe { cancel.as_ref() } {
        cancel.0.cancel();
    }
}

/// # Safety
/// `cancel` must be null or live, not concurrently accessed, and freed only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_cancel_free(cancel: *mut KlatterCancel) {
    if !cancel.is_null() {
        drop(unsafe { Box::from_raw(cancel) });
    }
}

/// # Safety
/// `utterance` must be null or live, exclusively accessed, and freed only once.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_utterance_free(utterance: *mut KlatterUtterance) {
    if !utterance.is_null() {
        drop(unsafe { Box::from_raw(utterance) });
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn quality_settings_are_described_and_change_the_voice() {
        assert_eq!(klatter_parameter_count(), engine::PARAMETERS.len());
        let mut info = std::mem::MaybeUninit::<KlatterParameterInfo>::uninit();
        assert_eq!(unsafe { klatter_parameter_info(0, info.as_mut_ptr()) }, 1);
        let info = unsafe { info.assume_init() };
        assert_eq!(unsafe { CStr::from_ptr(info.code) }.to_str().unwrap(), "fl");
        assert_eq!(info.default_value, 0.0);
        assert_eq!(unsafe { klatter_parameter_info(999, std::ptr::null_mut()) }, -1);

        let text = std::ffi::CString::new("Hello there.").unwrap();
        let texts = [text.as_ptr()];
        let voice = default_voice();
        let samples = |quality: &[f64]| unsafe {
            let s = klatter_speak_quality(texts.as_ptr(), std::ptr::null(), 1, 16000, &voice,
                                          quality.as_ptr(), quality.len());
            assert!(!s.is_null());
            let v = std::slice::from_raw_parts((*s).samples, (*s).sample_count).to_vec();
            klatter_free(s);
            v
        };
        let mut defaults = vec![0.0; klatter_parameter_count()];
        assert_eq!(unsafe { klatter_default_quality(defaults.as_mut_ptr(), defaults.len()) }, defaults.len());
        let plain = samples(&[]);
        assert_eq!(samples(&defaults), plain);
        assert_eq!(samples(&[f64::NAN; 3]), plain);
        assert_ne!(samples(&[25.0]), plain, "flutter 25");
        let mut big_head = defaults.clone();
        big_head[1] = 130.0;
        assert_ne!(samples(&big_head), plain);
    }

    use super::*;
    use std::ffi::CString;

    fn default_voice() -> KlatterVoice {
        let mut v = KlatterVoice {
            wpm: 0.0,
            pitch_hz: 0.0,
            inflection: 0.0,
            volume: 0.0,
        };
        unsafe { klatter_default_voice(&mut v) };
        v
    }

    #[test]
    fn pull_api_owns_inputs_snapshots_overrides_and_reports_exact_positions() {
        unsafe {
            let dictionary = klatter_pronunciations_create();
            assert!(!dictionary.is_null());
            assert_eq!(
                klatter_pronunciations_set(dictionary, c"read".as_ptr(), c"R EH1 D".as_ptr()),
                1
            );
            assert_eq!(
                klatter_pronunciations_set(dictionary, c"read".as_ptr(), c"R AE".as_ptr()),
                -1
            );
            let text = CString::new("Read $12.50.").unwrap();
            let texts = [text.as_ptr(), std::ptr::null()];
            let marks = [0, u32::MAX];
            let stream = klatter_utterance_create(
                texts.as_ptr(),
                marks.as_ptr(),
                2,
                22050,
                &default_voice(),
                dictionary,
            );
            assert!(!stream.is_null());
            drop(text);
            klatter_pronunciations_free(dictionary);
            let mut overrides = engine::Pronunciations::default();
            overrides.insert("read", "R EH1 D").unwrap();
            let expected = engine::speak_with_pronunciations(
                &[Item::Text("Read $12.50."), Item::Mark(u32::MAX)],
                22050,
                &Voice::default(),
                &overrides,
            );
            let mut pcm = Vec::new();
            let mut found_marks = Vec::new();
            let mut words = Vec::new();
            loop {
                let mut output = [123; 137];
                let mut block = KlatterPullBlock::default();
                let status =
                    klatter_utterance_read(stream, output.as_mut_ptr(), output.len(), &mut block);
                assert!(status >= 0);
                pcm.extend_from_slice(&output[..block.sample_count]);
                let ids = std::slice::from_raw_parts(block.mark_ids, block.mark_count);
                let samples = std::slice::from_raw_parts(block.mark_samples, block.mark_count);
                found_marks.extend(ids.iter().copied().zip(samples.iter().copied()));
                words.extend_from_slice(std::slice::from_raw_parts(block.words, block.word_count));
                if status == 0 {
                    break;
                }
            }
            assert_eq!(pcm, expected.samples);
            assert_eq!(found_marks, expected.marks);
            assert_eq!(words[0].byte_offset, 0);
            assert_eq!(words[0].byte_length, 4);
            assert!(
                words
                    .iter()
                    .any(|w| w.byte_offset == 5 && w.byte_length == 6)
            );
            let cancel = klatter_utterance_cancel_handle(stream);
            klatter_utterance_free(stream);
            klatter_cancel(cancel);
            klatter_cancel_free(cancel);
        }
    }

    #[test]
    fn pull_cancellation_and_invalid_inputs_have_distinct_statuses() {
        unsafe {
            let texts = [c"Hello.".as_ptr()];
            assert!(
                klatter_utterance_create(
                    texts.as_ptr(),
                    std::ptr::null(),
                    1,
                    16000,
                    std::ptr::null(),
                    std::ptr::null()
                )
                .is_null()
            );
            let stream = klatter_utterance_create(
                texts.as_ptr(),
                std::ptr::null(),
                1,
                16000,
                &default_voice(),
                std::ptr::null(),
            );
            let cancel = klatter_utterance_cancel_handle(stream);
            let mut block = KlatterPullBlock::default();
            assert_eq!(
                klatter_utterance_read(stream, std::ptr::null_mut(), 10, &mut block),
                -1
            );
            assert_eq!(
                klatter_utterance_read(stream, std::ptr::null_mut(), 0, &mut block),
                1
            );
            klatter_cancel(cancel);
            let mut output = [123; 32];
            assert_eq!(
                klatter_utterance_read(stream, output.as_mut_ptr(), 32, &mut block),
                -2
            );
            assert_eq!(output, [0; 32]);
            assert_eq!(block.sample_count, 0);
            assert_eq!(block.word_count, 0);
            klatter_cancel_free(cancel);
            klatter_utterance_free(stream);
            klatter_utterance_free(std::ptr::null_mut());
            klatter_cancel_free(std::ptr::null_mut());
            klatter_pronunciations_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn matches_the_engine_and_reports_marks() {
        let hello = CString::new("Hello there.").unwrap();
        let how = CString::new("How are you?").unwrap();
        let texts = [
            hello.as_ptr(),
            std::ptr::null(),
            how.as_ptr(),
            std::ptr::null(),
        ];
        let marks = [0, 1, 0, 2];
        let speech =
            unsafe { klatter_speak(texts.as_ptr(), marks.as_ptr(), 4, 16000, &default_voice()) };
        assert!(!speech.is_null());
        let s = unsafe { &*speech };
        let samples = unsafe { std::slice::from_raw_parts(s.samples, s.sample_count) };
        let expected = engine::speak(
            &[
                Item::Text("Hello there."),
                Item::Mark(1),
                Item::Text("How are you?"),
                Item::Mark(2),
            ],
            16000,
            &Voice::default(),
        );
        assert_eq!(samples, expected.samples.as_slice());
        let ids = unsafe { std::slice::from_raw_parts(s.mark_ids, s.mark_count) };
        let at = unsafe { std::slice::from_raw_parts(s.mark_samples, s.mark_count) };
        let marks: Vec<(u32, usize)> = ids.iter().copied().zip(at.iter().copied()).collect();
        assert_eq!(marks, expected.marks);
        unsafe { klatter_free(speech) };
    }

    #[test]
    fn rejects_invalid_input() {
        let ok = CString::new("hi").unwrap();
        let texts = [ok.as_ptr()];
        let bad_voice = KlatterVoice {
            wpm: 0.0,
            ..default_voice()
        };
        assert!(
            unsafe { klatter_speak(texts.as_ptr(), std::ptr::null(), 1, 16000, &bad_voice) }
                .is_null()
        );
        assert!(
            unsafe { klatter_speak(texts.as_ptr(), std::ptr::null(), 1, 0, &default_voice()) }
                .is_null()
        );
        assert!(
            unsafe { klatter_speak(texts.as_ptr(), std::ptr::null(), 1, 16000, std::ptr::null()) }
                .is_null()
        );
        let mark_only = [std::ptr::null()];
        assert!(
            unsafe {
                klatter_speak(
                    mark_only.as_ptr(),
                    std::ptr::null(),
                    1,
                    16000,
                    &default_voice(),
                )
            }
            .is_null()
        );
        let invalid_utf8 = [c"\xff".as_ptr()];
        assert!(
            unsafe {
                klatter_speak(
                    invalid_utf8.as_ptr(),
                    std::ptr::null(),
                    1,
                    16000,
                    &default_voice(),
                )
            }
            .is_null()
        );
        unsafe { klatter_free(std::ptr::null_mut()) };
    }

    #[test]
    fn empty_input_is_valid() {
        let speech = unsafe {
            klatter_speak(
                std::ptr::null(),
                std::ptr::null(),
                0,
                16000,
                &default_voice(),
            )
        };
        assert!(!speech.is_null());
        assert_eq!(unsafe { (*speech).mark_count }, 0);
        unsafe { klatter_free(speech) };
    }

    #[derive(Default)]
    struct StreamState {
        speech: engine::Speech,
        cancel_on_audio: bool,
        audio_blocks: usize,
        valid_offsets: bool,
    }

    unsafe extern "C" fn collect(block: *const KlatterSpeech, context: *mut c_void) -> c_int {
        let state = unsafe { &mut *context.cast::<StreamState>() };
        let block = unsafe { &*block };
        let samples = unsafe { std::slice::from_raw_parts(block.samples, block.sample_count) };
        let ids = unsafe { std::slice::from_raw_parts(block.mark_ids, block.mark_count) };
        let offsets = unsafe { std::slice::from_raw_parts(block.mark_samples, block.mark_count) };
        let start = state.speech.samples.len();
        state.valid_offsets &= offsets.iter().all(|&at| at <= block.sample_count);
        state
            .speech
            .marks
            .extend(ids.iter().zip(offsets).map(|(&id, &at)| (id, start + at)));
        state.speech.samples.extend_from_slice(samples);
        if !samples.is_empty() {
            state.audio_blocks += 1;
            if state.cancel_on_audio {
                return 0;
            }
        }
        1
    }

    #[test]
    fn streaming_c_api_matches_owned_speech_and_cancels() {
        let texts = [
            std::ptr::null(),
            c"Hello there.".as_ptr(),
            std::ptr::null(),
            c"How are you?".as_ptr(),
            std::ptr::null(),
        ];
        let marks = [1, 0, 2, 0, 3];
        let expected = engine::speak(
            &[
                Item::Mark(1),
                Item::Text("Hello there."),
                Item::Mark(2),
                Item::Text("How are you?"),
                Item::Mark(3),
            ],
            22050,
            &Voice::default(),
        );
        for cancel in [false, true] {
            let mut state = StreamState {
                cancel_on_audio: cancel,
                valid_offsets: true,
                ..StreamState::default()
            };
            let status = unsafe {
                klatter_speak_stream(
                    texts.as_ptr(),
                    marks.as_ptr(),
                    texts.len(),
                    22050,
                    &default_voice(),
                    Some(collect),
                    (&mut state as *mut StreamState).cast(),
                )
            };
            assert!(state.valid_offsets);
            if cancel {
                assert_eq!(status, 0);
                assert_eq!(state.audio_blocks, 1);
                assert_eq!(state.speech.samples, expected.samples[..2205]);
            } else {
                assert_eq!(status, 1);
                assert_eq!(state.speech, expected);
            }
        }
    }

    #[test]
    fn streaming_rejects_invalid_input_without_calling_back() {
        let mut state = StreamState::default();
        let status = unsafe {
            klatter_speak_stream(
                std::ptr::null(),
                std::ptr::null(),
                1,
                16000,
                &default_voice(),
                Some(collect),
                (&mut state as *mut StreamState).cast(),
            )
        };
        assert_eq!(status, -1);
        assert_eq!(state.audio_blocks, 0);
        assert_eq!(
            unsafe {
                klatter_speak_stream(
                    std::ptr::null(),
                    std::ptr::null(),
                    0,
                    16000,
                    &default_voice(),
                    None,
                    std::ptr::null_mut(),
                )
            },
            -1
        );
    }
}

// ---- Voice quality -----------------------------------------------------------

/// The original entry points speak with the default voice quality.
///
/// # Safety
/// As [`klatter_speak_quality`] with no quality settings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_speak(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
) -> *mut KlatterSpeech {
    unsafe { klatter_speak_quality(texts, marks, count, sample_rate, voice, std::ptr::null(), 0) }
}

/// # Safety
/// As [`klatter_speak_stream_quality`] with no quality settings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_speak_stream(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
    callback: Option<SpeechCallback>,
    context: *mut c_void,
) -> c_int {
    unsafe {
        klatter_speak_stream_quality(texts, marks, count, sample_rate, voice, std::ptr::null(), 0, callback, context)
    }
}

/// # Safety
/// As [`klatter_utterance_create_quality`] with no quality settings.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_utterance_create(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
    dictionary: *const KlatterPronunciations,
) -> *mut KlatterUtterance {
    unsafe {
        klatter_utterance_create_quality(texts, marks, count, sample_rate, voice, std::ptr::null(), 0, dictionary)
    }
}

/// One voice-quality setting, as `klatter_parameter_info` describes it. The
/// strings are static, NUL-terminated UTF-8.
#[repr(C)]
pub struct KlatterParameterInfo {
    pub code: *const c_char,
    pub name: *const c_char,
    pub unit: *const c_char,
    pub help: *const c_char,
    pub min: f64,
    pub max: f64,
    pub default_value: f64,
    pub step: f64,
}

/// The number of voice-quality settings, the length of a full `quality` array.
#[unsafe(no_mangle)]
pub extern "C" fn klatter_parameter_count() -> usize {
    engine::PARAMETERS.len()
}

fn static_cstr(text: &'static str) -> *const c_char {
    use std::collections::HashMap;
    use std::ffi::CString;
    use std::sync::{Mutex, OnceLock};
    static STRINGS: OnceLock<Mutex<HashMap<&'static str, &'static CString>>> = OnceLock::new();
    let mut map = STRINGS.get_or_init(Default::default).lock().unwrap_or_else(|e| e.into_inner());
    map.entry(text)
        .or_insert_with(|| Box::leak(Box::new(CString::new(text).unwrap_or_default())))
        .as_ptr()
}

/// Describes setting `index`. Returns 1, or -1 for an index out of range or a
/// null `out`.
///
/// # Safety
/// `out` must be null or point to writable memory for one `KlatterParameterInfo`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_parameter_info(index: usize, out: *mut KlatterParameterInfo) -> c_int {
    let (Some(p), Some(out)) = (engine::PARAMETERS.get(index), unsafe { out.as_mut() }) else {
        return -1;
    };
    *out = KlatterParameterInfo {
        code: static_cstr(p.code),
        name: static_cstr(p.name),
        unit: static_cstr(p.unit),
        help: static_cstr(p.help),
        min: p.min,
        max: p.max,
        default_value: p.default,
        step: p.step,
    };
    1
}

/// Fills `out` with every setting's default, up to `count` values; returns how
/// many it wrote.
///
/// # Safety
/// `out` must point to `count` writable doubles.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_default_quality(out: *mut f64, count: usize) -> usize {
    if out.is_null() {
        return 0;
    }
    let n = count.min(engine::PARAMETERS.len());
    for (i, p) in engine::PARAMETERS.iter().take(n).enumerate() {
        unsafe { *out.add(i) = p.default };
    }
    n
}
