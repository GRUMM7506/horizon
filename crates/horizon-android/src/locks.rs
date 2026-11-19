//! File locks on storage that has none. The game's folder is the app's own folder on shared
//! storage (`getExternalFilesDir`), which Android serves through FUSE, and `flock` there
//! answers ENOSYS. The game locks files to keep a second copy of itself out (a world's
//! `session.lock`, its log, the pack cache) and takes a failed lock for "someone else has it":
//! no world would open. There is one game per device here, in one process, so on storage
//! without locks a lock is simply granted.
//!
//! As with the thread's stack (stack.rs), the library is linked with `--wrap=flock`: std's
//! `File::lock` and `try_lock` come through here.
use std::ffi::c_int;
use std::sync::atomic::{AtomicBool, Ordering};

unsafe extern "C" {
    /// The real `flock`, as the linker's `--wrap` names it.
    fn __real_flock(fd: c_int, operation: c_int) -> c_int;
}

static TOLD: AtomicBool = AtomicBool::new(false);

/// # Safety
/// `flock`'s contract: `fd` is an open file descriptor of the caller's.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn __wrap_flock(fd: c_int, operation: c_int) -> c_int {
    // SAFETY: the caller's own arguments, passed on; errno is the calling thread's.
    unsafe {
        let result = __real_flock(fd, operation);
        if result == 0 {
            return 0;
        }
        let errno = *libc::__errno();
        if errno != libc::ENOSYS && errno != libc::EOPNOTSUPP {
            return result;
        }
        *libc::__errno() = 0;
    }
    if !TOLD.swap(true, Ordering::Relaxed) {
        eprintln!("Horizon for Android: the game's storage has no file locks; they are taken as granted");
    }
    0
}
