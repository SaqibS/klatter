//! Klatter as WebAssembly, for the page in `www/`.
//!
//! No bindings library: the page writes text and settings into memory from
//! [`klatter_alloc`], and the engine hands each block of audio back as it is
//! made through the one function the page supplies, `env.klatter_audio`, which
//! gets 16-bit samples and returns 0 to stop. Speech runs in a Web Worker.

use klatter::{Item, PARAMETERS, Voice, speak_stream};
use std::cell::RefCell;

#[cfg(target_arch = "wasm32")]
#[link(wasm_import_module = "env")]
unsafe extern "C" {
    fn klatter_audio(samples: *const i16, count: usize) -> u32;
}

#[cfg(not(target_arch = "wasm32"))]
unsafe fn klatter_audio(_samples: *const i16, _count: usize) -> u32 {
    1
}

thread_local! {
    static MESSAGE: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

fn set_message(text: &str) {
    MESSAGE.with(|m| {
        let mut m = m.borrow_mut();
        m.clear();
        m.extend_from_slice(text.as_bytes());
    });
}

/// Memory for the page to write text or settings into, aligned for doubles.
#[unsafe(no_mangle)]
pub extern "C" fn klatter_alloc(size: usize) -> *mut u8 {
    match std::alloc::Layout::from_size_align(size.max(1), 8) {
        Ok(layout) => unsafe { std::alloc::alloc(layout) },
        Err(_) => std::ptr::null_mut(),
    }
}

/// # Safety
/// `pointer` and `size` must come from one call to [`klatter_alloc`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn klatter_free(pointer: *mut u8, size: usize) {
    if let (false, Ok(layout)) = (pointer.is_null(), std::alloc::Layout::from_size_align(size.max(1), 8)) {
        unsafe { std::alloc::dealloc(pointer, layout) };
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn klatter_message() -> *const u8 {
    MESSAGE.with(|m| m.borrow().as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn klatter_message_length() -> usize {
    MESSAGE.with(|m| m.borrow().len())
}

fn json_string(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// Puts the voice-quality settings in the message, as JSON, and returns how many.
#[unsafe(no_mangle)]
pub extern "C" fn klatter_parameters() -> usize {
    let items: Vec<String> = PARAMETERS
        .iter()
        .map(|p| {
            format!(
                "{{\"code\":{},\"name\":{},\"unit\":{},\"help\":{},\"min\":{},\"max\":{},\"default\":{},\"step\":{}}}",
                json_string(p.code), json_string(p.name), json_string(p.unit), json_string(p.help),
                p.min, p.max, p.default, p.step
            )
        })
        .collect();
    set_message(&format!("[{}]", items.join(",")));
    PARAMETERS.len()
}

/// Speaks UTF-8 text with these settings, handing audio to `klatter_audio`.
/// `quality` points to `quality_count` doubles in the order of
/// [`klatter_parameters`]. Returns 0 when all was spoken, 1 when stopped.
///
/// # Safety
/// `text` must point to `length` bytes and `quality` to `quality_count` doubles.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn klatter_say(
    text: *const u8,
    length: usize,
    wpm: f64,
    pitch_hz: f64,
    inflection: f64,
    volume: f64,
    sample_rate: u32,
    quality: *const f64,
    quality_count: usize,
) -> i32 {
    let bytes = if length == 0 { &[][..] } else { unsafe { std::slice::from_raw_parts(text, length) } };
    let text = String::from_utf8_lossy(bytes);
    let mut voice = Voice {
        wpm: wpm.clamp(30.0, 1920.0),
        pitch_hz: pitch_hz.clamp(40.0, 400.0),
        inflection: inflection.clamp(0.0, 2.0),
        volume: volume.clamp(0.0, 2.0),
        ..Voice::default()
    };
    if !quality.is_null() {
        for i in 0..quality_count.min(PARAMETERS.len()) {
            voice.quality.set_index(i, unsafe { *quality.add(i) });
        }
    }
    let done = speak_stream(&[Item::Text(&text)], sample_rate.clamp(8000, 96000), &voice, |samples, _| {
        samples.is_empty() || unsafe { klatter_audio(samples.as_ptr(), samples.len()) != 0 }
    });
    if done { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lists_the_settings_as_json() {
        assert_eq!(klatter_parameters(), PARAMETERS.len());
        let json = MESSAGE.with(|m| String::from_utf8(m.borrow().clone()).unwrap());
        assert!(json.starts_with("[{\"code\":\"fl\",\"name\":\"Flutter\""));
    }

    #[test]
    fn speaks() {
        let text = "Hello.";
        let q = [25.0];
        assert_eq!(unsafe { klatter_say(text.as_ptr(), text.len(), 219.0, 100.0, 1.0, 1.0, 16000, q.as_ptr(), 1) }, 0);
    }
}
