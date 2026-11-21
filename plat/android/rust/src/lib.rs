//! Klatter for Android: a static library the JNI bridge (../jni) links into
//! libklatter.so. It holds the engine's C interface, the functions of
//! ffi/klatter.h, and a memory allocator that older Android has the functions for.

// The C interface's functions, kept in the library for the bridge to call.
pub use klatter::*;

/// Rust's own allocator asks for posix_memalign, which Android only has from
/// 4.2 (API 17): a library that needs it will not load on Android 4.0 and 4.1.
/// This one uses malloc, and memalign for the rare larger alignment, which
/// every Android has had.
#[cfg(target_os = "android")]
mod old_android_allocator {
    use std::alloc::{GlobalAlloc, Layout};
    use std::ffi::c_void;

    unsafe extern "C" {
        fn malloc(size: usize) -> *mut c_void;
        fn calloc(count: usize, size: usize) -> *mut c_void;
        fn realloc(pointer: *mut c_void, size: usize) -> *mut c_void;
        fn memalign(alignment: usize, size: usize) -> *mut c_void;
        fn free(pointer: *mut c_void);
    }

    /// What malloc's blocks are always aligned to on Android.
    const MALLOC_ALIGN: usize = if cfg!(target_pointer_width = "64") { 16 } else { 8 };

    struct OldAndroidAllocator;

    unsafe impl GlobalAlloc for OldAndroidAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            unsafe {
                if layout.align() <= MALLOC_ALIGN && layout.align() <= layout.size() {
                    malloc(layout.size()) as *mut u8
                } else {
                    memalign(layout.align().max(size_of::<usize>()), layout.size()) as *mut u8
                }
            }
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            unsafe {
                if layout.align() <= MALLOC_ALIGN && layout.align() <= layout.size() {
                    calloc(layout.size(), 1) as *mut u8
                } else {
                    let p = self.alloc(layout);
                    if !p.is_null() {
                        std::ptr::write_bytes(p, 0, layout.size());
                    }
                    p
                }
            }
        }

        unsafe fn dealloc(&self, pointer: *mut u8, _: Layout) {
            unsafe { free(pointer as *mut c_void) }
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            unsafe {
                if layout.align() <= MALLOC_ALIGN && layout.align() <= new_size {
                    realloc(pointer as *mut c_void, new_size) as *mut u8
                } else {
                    let new = self.alloc(Layout::from_size_align_unchecked(new_size, layout.align()));
                    if !new.is_null() {
                        std::ptr::copy_nonoverlapping(pointer, new, layout.size().min(new_size));
                        self.dealloc(pointer, layout);
                    }
                    new
                }
            }
        }
    }

    #[global_allocator]
    static ALLOCATOR: OldAndroidAllocator = OldAndroidAllocator;
}
