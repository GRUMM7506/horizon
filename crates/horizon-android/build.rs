//! Android only: every `pthread_create` of the library goes through `__wrap_pthread_create`
//! (src/stack.rs), which gives the activity's native thread the game's main-thread stack, and
//! every `flock` through `__wrap_flock` (src/locks.rs), for storage that has no file locks.
fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("android") {
        println!("cargo:rustc-cdylib-link-arg=-Wl,--wrap=pthread_create");
        println!("cargo:rustc-cdylib-link-arg=-Wl,--wrap=flock");
    }
}
