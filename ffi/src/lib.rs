//! C interface to the engine, declared in `klatter.h`, for hosts that are not
//! written in Rust.

use std::ffi::{CStr, c_char};
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
pub unsafe extern "C" fn klatter_speak(
    texts: *const *const c_char,
    marks: *const u32,
    count: usize,
    sample_rate: u32,
    voice: *const KlatterVoice,
) -> *mut KlatterSpeech {
    let Some(v) = (unsafe { voice.as_ref() }).copied() else {
        return std::ptr::null_mut();
    };
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
        return std::ptr::null_mut();
    }
    let mut items = Vec::with_capacity(count);
    for i in 0..count {
        let text = unsafe { *texts.add(i) };
        if text.is_null() {
            if marks.is_null() {
                return std::ptr::null_mut();
            }
            items.push(Item::Mark(unsafe { *marks.add(i) }));
        } else {
            let Ok(text) = unsafe { CStr::from_ptr(text) }.to_str() else {
                return std::ptr::null_mut();
            };
            items.push(Item::Text(text));
        }
    }
    let voice = Voice {
        wpm: v.wpm,
        pitch_hz: v.pitch_hz,
        inflection: v.inflection,
        volume: v.volume,
    };
    // Unwinding across the C boundary is undefined behaviour.
    let Ok(speech) = catch_unwind(AssertUnwindSafe(|| {
        engine::speak(&items, sample_rate, &voice)
    })) else {
        return std::ptr::null_mut();
    };
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

/// # Safety
/// `speech` must be null or a pointer returned by `klatter_speak` that has
/// not been freed.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_free(speech: *mut KlatterSpeech) {
    if !speech.is_null() {
        drop(unsafe { Box::from_raw(speech.cast::<OwnedSpeech>()) });
    }
}

#[cfg(test)]
mod tests {
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
}
