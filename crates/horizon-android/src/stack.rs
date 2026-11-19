//! The stack of the activity's native thread. android-activity's glue starts it with bionic's
//! default (1 MiB), where the game's main thread has `GUEST_STACK_BYTES + HOST_HEADROOM`
//! everywhere else (executable link settings on Windows and macOS, the stack limit on Linux) and
//! refuses to enter a mod's guest code on less (`horizon_guest_stack::check_current`).
//!
//! The glue is C and makes the thread itself, so the library is linked with
//! `--wrap=pthread_create` (build.rs): every `pthread_create` in it comes here, and the one that
//! asks for a detached thread of the default size, which only the glue does (Rust's threads are
//! joinable and name their size), gets the game's reservation instead.
use std::ffi::{c_int, c_void};

type Start = extern "C" fn(*mut c_void) -> *mut c_void;

unsafe extern "C" {
    /// The real `pthread_create`, as the linker's `--wrap` names it.
    fn __real_pthread_create(
        thread: *mut libc::pthread_t,
        attr: *const libc::pthread_attr_t,
        start: Start,
        arg: *mut c_void,
    ) -> c_int;
    /// Bionic's, which the libc crate doesn't declare.
    fn pthread_attr_getdetachstate(attr: *const libc::pthread_attr_t, state: *mut c_int) -> c_int;
}

/// Larger than this was asked for on purpose.
const DEFAULT_STACK_LIMIT: usize = 2 * 1024 * 1024;

/// # Safety
/// `pthread_create`'s contract: the caller's pointers are what it would pass to that.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_pthread_create(
    thread: *mut libc::pthread_t,
    attr: *const libc::pthread_attr_t,
    start: Start,
    arg: *mut c_void,
) -> c_int {
    // SAFETY: `attr` is the caller's initialized attributes when not null; `own` is initialized
    // by pthread_attr_init before any other use and destroyed after.
    unsafe {
        if !attr.is_null() {
            let mut detach = 0;
            let mut size = 0;
            if pthread_attr_getdetachstate(attr, &mut detach) == 0
                && detach == libc::PTHREAD_CREATE_DETACHED
                && libc::pthread_attr_getstacksize(attr, &mut size) == 0
                && size < DEFAULT_STACK_LIMIT
            {
                let mut own = std::mem::zeroed::<libc::pthread_attr_t>();
                if libc::pthread_attr_init(&mut own) == 0 {
                    libc::pthread_attr_setdetachstate(&mut own, libc::PTHREAD_CREATE_DETACHED);
                    let wanted =
                        horizon_guest_stack::GUEST_STACK_BYTES + horizon_guest_stack::HOST_HEADROOM;
                    let created = libc::pthread_attr_setstacksize(&mut own, wanted) == 0
                        && __real_pthread_create(thread, &own, start, arg) == 0;
                    libc::pthread_attr_destroy(&mut own);
                    if created {
                        return 0;
                    }
                }
            }
        }
        __real_pthread_create(thread, attr, start, arg)
    }
}

/// The calling thread's stack reservation in bytes, 0 when the system doesn't say.
pub fn current_size() -> usize {
    // SAFETY: pthread_self is live and `attr` is writable; destroyed only once initialized.
    unsafe {
        let mut attr = std::mem::zeroed::<libc::pthread_attr_t>();
        if libc::pthread_getattr_np(libc::pthread_self(), &mut attr) != 0 {
            return 0;
        }
        let mut size = 0;
        let read = libc::pthread_attr_getstacksize(&attr, &mut size);
        libc::pthread_attr_destroy(&mut attr);
        if read == 0 { size } else { 0 }
    }
}
