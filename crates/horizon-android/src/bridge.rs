//! The JNI side: what the game asks of the activity (`GameActivity` in android/, by method
//! name), and what the activity's views hand the game (`GameBridge`'s native methods). The
//! activity calls on its UI thread; everything lands in `horizon_client_input::android` or
//! `horizon_client_render::android_runtime`, which the game reads on its own thread.
use horizon_client_input::android::{self as game, Event};
use horizon_client_render::android_runtime;
use jni::objects::{GlobalRef, JClass, JIntArray, JObject, JString, JValue};
use jni::sys::{jboolean, jfloat, jint};
use jni::{JNIEnv, JavaVM};
use std::sync::OnceLock;
use winit::event::{DeviceEvent, ElementState};
use winit::platform::android::activity::AndroidApp;

static ACTIVITY: OnceLock<(JavaVM, GlobalRef)> = OnceLock::new();

/// Keeps the activity for the game's requests, and registers them with the game.
pub fn install(app: &AndroidApp) {
    // SAFETY: `vm_as_ptr` is the process's JavaVM and `activity_as_ptr` a reference to the
    // activity that android-activity keeps valid while `android_main` runs; a global reference
    // of our own is taken at once, and the borrowed one is not deleted.
    let kept = unsafe {
        JavaVM::from_raw(app.vm_as_ptr().cast())
            .ok()
            .and_then(|vm| {
                let activity = {
                    let env = vm.attach_current_thread_permanently().ok()?;
                    let activity = JObject::from_raw(app.activity_as_ptr().cast());
                    env.new_global_ref(&activity).ok()?
                };
                Some((vm, activity))
            })
    };
    match kept {
        Some(kept) => {
            let _ = ACTIVITY.set(kept);
        }
        None => eprintln!("Horizon for Android: the activity could not be reached over JNI"),
    }
    game::set_keyboard(|show| call("showKeyboard", show));
    game::set_pointer_capture(|capture| call("capturePointer", capture));
}

/// Calls the activity's `name(boolean)`. A missing method or a Java exception only logs.
fn call(name: &str, value: bool) {
    let Some((vm, activity)) = ACTIVITY.get() else {
        return;
    };
    let Ok(mut env) = vm.attach_current_thread_permanently() else {
        return;
    };
    if env
        .call_method(activity, name, "(Z)V", &[JValue::Bool(value.into())])
        .is_err()
    {
        let _ = env.exception_describe();
        let _ = env.exception_clear();
    }
}

/// The game ended: the activity closes.
pub fn finish(_app: &AndroidApp) {
    call("gameEnded", true);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_text<'local>(
    mut env: JNIEnv<'local>,
    _class: JClass<'local>,
    text: JString<'local>,
) {
    if let Ok(text) = env.get_string(&text) {
        game::push(Event::Text(text.into()));
    }
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_backspace(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    game::push(Event::Backspace);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_enter(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    game::push(Event::Enter);
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_keyboardHidden(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
) {
    game::keyboard_hidden();
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_mouseMoved(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    x: jfloat,
    y: jfloat,
) {
    game::push(Event::MouseMoved([x, y]));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_mouseMotion(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    x: jfloat,
    y: jfloat,
) {
    game::push(Event::MouseMotion([x, y]));
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_mouseButton(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    button: jint,
    down: jboolean,
    x: jfloat,
    y: jfloat,
) {
    game::push(Event::MouseButton {
        button: button.clamp(0, 2) as u8,
        down: down != 0,
        at: [x, y],
    });
}

#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_mouseWheel(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    x: jfloat,
    y: jfloat,
) {
    game::push(Event::MouseWheel([x, y]));
}

/// A controller's axis (`MotionEvent.AXIS_*`) moved.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_gamepadAxis(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    device: jint,
    axis: jint,
    value: jfloat,
) {
    android_runtime::gamepad_event(
        device,
        &DeviceEvent::Motion {
            axis: axis.max(0) as u32,
            value: value.into(),
        },
    );
}

/// A controller's button (`KeyEvent.KEYCODE_*`) went down or up.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_gamepadButton(
    _env: JNIEnv<'_>,
    _class: JClass<'_>,
    device: jint,
    code: jint,
    down: jboolean,
) {
    let state = if down != 0 {
        ElementState::Pressed
    } else {
        ElementState::Released
    };
    android_runtime::gamepad_event(
        device,
        &DeviceEvent::Button {
            button: code.max(0) as u32,
            state,
        },
    );
}

/// The controllers connected now, by Android device id: the others are forgotten.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_horizonport_android_GameBridge_gamepads<'local>(
    env: JNIEnv<'local>,
    _class: JClass<'local>,
    ids: JIntArray<'local>,
) {
    let length = env.get_array_length(&ids).unwrap_or(0).max(0) as usize;
    let mut connected = vec![0; length];
    if env.get_int_array_region(&ids, 0, &mut connected).is_ok() {
        android_runtime::retain_gamepads(&connected);
    }
}
