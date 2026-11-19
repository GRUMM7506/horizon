package dev.horizonport.android

import androidx.annotation.Keep

/**
 * The game's native side (crates/horizon-android/src/bridge.rs). The library is loaded by
 * [GameActivity]; these only hand the game what its own views receive.
 */
@Keep
object GameBridge {
    @JvmStatic external fun text(text: String)
    @JvmStatic external fun backspace()
    @JvmStatic external fun enter()
    @JvmStatic external fun keyboardHidden()
    @JvmStatic external fun mouseMoved(x: Float, y: Float)
    @JvmStatic external fun mouseMotion(x: Float, y: Float)
    @JvmStatic external fun mouseButton(button: Int, down: Boolean, x: Float, y: Float)
    @JvmStatic external fun mouseWheel(x: Float, y: Float)
    @JvmStatic external fun gamepadAxis(device: Int, axis: Int, value: Float)
    @JvmStatic external fun gamepadButton(device: Int, code: Int, down: Boolean)
    @JvmStatic external fun gamepads(ids: IntArray)
}
