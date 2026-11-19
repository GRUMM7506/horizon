package dev.horizonport.android

import android.content.Context
import android.hardware.input.InputManager
import android.os.Build
import android.os.Bundle
import android.system.Os
import android.view.InputDevice
import android.view.KeyEvent
import android.view.MotionEvent
import android.view.View
import android.view.ViewGroup
import android.view.WindowManager
import android.view.inputmethod.InputMethodManager
import androidx.annotation.Keep
import androidx.core.view.ViewCompat
import androidx.core.view.WindowCompat
import androidx.core.view.WindowInsetsCompat
import androidx.core.view.WindowInsetsControllerCompat
import com.google.androidgamesdk.GameActivity

/**
 * The activity the game runs in. `GameActivity` loads `libhorizon_android.so` and gives it the
 * surface, touches and keys; this adds what the game needs around that: its folder and arguments
 * (through the environment, before the library loads), the full screen, the on-screen keyboard's
 * text, a mouse and controllers. It runs in its own process (`:game`), which ends with the game.
 */
class HorizonActivity : GameActivity(), InputManager.InputDeviceListener {
    private lateinit var keyboard: KeyboardSink
    private var keyboardWanted = false
    private var keyboardSeen = false
    private var captureWanted = false
    private var mouseButtons = 0

    override fun onCreate(savedInstanceState: Bundle?) {
        val dir = intent.getStringExtra(EXTRA_DIR) ?: GameFiles.dir(this).path
        Os.setenv("HORIZON_ANDROID_DIR", dir, true)
        Os.setenv("HORIZON_ANDROID_ARGS", intent.getStringArrayExtra(EXTRA_ARGS).orEmpty().joinToString("\n"), true)
        // Rust's temp_dir and the game's fallbacks: both writable, both the app's own.
        Os.setenv("TMPDIR", cacheDir.path, true)
        Os.setenv("HOME", dir, true)
        for (pair in intent.getStringArrayExtra(EXTRA_ENV).orEmpty()) {
            val name = pair.substringBefore('=')
            if (name.isNotEmpty()) Os.setenv(name, pair.substringAfter('=', ""), true)
        }
        if (Build.VERSION.SDK_INT >= 28) {
            window.attributes = window.attributes.apply {
                layoutInDisplayCutoutMode = if (intent.getBooleanExtra(EXTRA_UNDER_CUTOUT, false)) {
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_SHORT_EDGES
                } else {
                    WindowManager.LayoutParams.LAYOUT_IN_DISPLAY_CUTOUT_MODE_NEVER
                }
            }
        }
        super.onCreate(savedInstanceState)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        fullScreen()

        keyboard = KeyboardSink(this)
        addContentView(keyboard, ViewGroup.LayoutParams(1, 1))
        // The surface ends above the on-screen keyboard, so the chat line stays in sight.
        val content = findViewById<View>(android.R.id.content)
        ViewCompat.setOnApplyWindowInsetsListener(content) { view, insets ->
            val shown = insets.isVisible(WindowInsetsCompat.Type.ime())
            view.setPadding(0, 0, 0, insets.getInsets(WindowInsetsCompat.Type.ime()).bottom)
            if (keyboardWanted && keyboardSeen && !shown) {
                // The player sent the keyboard away: a touch on the game brings it back.
                GameBridge.keyboardHidden()
                keyboardWanted = false
            }
            keyboardSeen = shown
            insets
        }
        mSurfaceView.setOnCapturedPointerListener { _, event -> capturedPointer(event) }
        (getSystemService(Context.INPUT_SERVICE) as InputManager).registerInputDeviceListener(this, null)
    }

    override fun onDestroy() {
        (getSystemService(Context.INPUT_SERVICE) as InputManager).unregisterInputDeviceListener(this)
        super.onDestroy()
    }

    private fun fullScreen() {
        WindowCompat.setDecorFitsSystemWindows(window, false)
        WindowInsetsControllerCompat(window, window.decorView).apply {
            hide(WindowInsetsCompat.Type.systemBars())
            systemBarsBehavior = WindowInsetsControllerCompat.BEHAVIOR_SHOW_TRANSIENT_BARS_BY_SWIPE
        }
    }

    override fun onWindowFocusChanged(hasFocus: Boolean) {
        super.onWindowFocusChanged(hasFocus)
        if (hasFocus) {
            fullScreen()
            if (captureWanted) mSurfaceView.requestPointerCapture()
        }
    }

    // --- What the game asks for (crates/horizon-android/src/bridge.rs), on its own thread. ---

    /** A text field of the game took or lost the focus. */
    @Keep
    @Suppress("unused")
    fun showKeyboard(show: Boolean) = runOnUiThread {
        val input = getSystemService(Context.INPUT_METHOD_SERVICE) as InputMethodManager
        keyboardWanted = show
        if (show) {
            keyboard.reset()
            keyboard.requestFocus()
            input.showSoftInput(keyboard, 0)
        } else {
            keyboardSeen = false
            input.hideSoftInputFromWindow(keyboard.windowToken, 0)
            mSurfaceView.requestFocus()
        }
    }

    /** The player looks around with a mouse (captured), or a screen needs its pointer back. */
    @Keep
    @Suppress("unused")
    fun capturePointer(capture: Boolean) = runOnUiThread {
        captureWanted = capture
        if (capture) {
            if (!keyboardWanted) mSurfaceView.requestFocus()
            mSurfaceView.requestPointerCapture()
        } else {
            mSurfaceView.releasePointerCapture()
        }
    }

    /** The game returned: its process ends right after this. */
    @Keep
    @Suppress("unused", "UNUSED_PARAMETER")
    fun gameEnded(ended: Boolean) = runOnUiThread { finish() }

    // --- A mouse. winit's Android backend would report it as a finger. ---

    private fun mouse(event: MotionEvent) = event.getToolType(0) == MotionEvent.TOOL_TYPE_MOUSE

    private fun mouseButtons(event: MotionEvent, x: Float, y: Float) {
        val now = if (event.actionMasked == MotionEvent.ACTION_CANCEL) 0 else event.buttonState
        val buttons = intArrayOf(
            MotionEvent.BUTTON_PRIMARY,
            MotionEvent.BUTTON_SECONDARY,
            MotionEvent.BUTTON_TERTIARY,
        )
        for ((index, mask) in buttons.withIndex()) {
            val down = now and mask != 0
            if (down != (mouseButtons and mask != 0)) GameBridge.mouseButton(index, down, x, y)
        }
        mouseButtons = now
    }

    override fun onTouchEvent(event: MotionEvent): Boolean {
        if (!mouse(event)) return super.onTouchEvent(event)
        GameBridge.mouseMoved(event.x, event.y)
        mouseButtons(event, event.x, event.y)
        return true
    }

    private fun capturedPointer(event: MotionEvent): Boolean {
        // A captured mouse reports how far it moved; a touchpad reports positions, which this
        // leaves alone.
        if (event.source and InputDevice.SOURCE_MOUSE_RELATIVE == InputDevice.SOURCE_MOUSE_RELATIVE) {
            var x = event.x
            var y = event.y
            for (i in 0 until event.historySize) {
                x += event.getHistoricalX(i)
                y += event.getHistoricalY(i)
            }
            if (x != 0f || y != 0f) GameBridge.mouseMotion(x, y)
            mouseButtons(event, -1f, -1f)
            if (event.actionMasked == MotionEvent.ACTION_SCROLL) wheel(event)
        }
        return true
    }

    private fun wheel(event: MotionEvent) =
        GameBridge.mouseWheel(event.getAxisValue(MotionEvent.AXIS_HSCROLL), event.getAxisValue(MotionEvent.AXIS_VSCROLL))

    // --- Controllers: Android's key codes and axes, decoded by the game (android_gamepad.rs). ---

    private fun fromPad(source: Int) =
        source and InputDevice.SOURCE_GAMEPAD == InputDevice.SOURCE_GAMEPAD ||
            source and InputDevice.SOURCE_JOYSTICK == InputDevice.SOURCE_JOYSTICK

    override fun onGenericMotionEvent(event: MotionEvent): Boolean {
        if (fromPad(event.source) && event.actionMasked == MotionEvent.ACTION_MOVE) {
            for (axis in PAD_AXES) GameBridge.gamepadAxis(event.deviceId, axis, event.getAxisValue(axis))
            return true
        }
        if (event.isFromSource(InputDevice.SOURCE_MOUSE)) {
            when (event.actionMasked) {
                MotionEvent.ACTION_HOVER_MOVE -> GameBridge.mouseMoved(event.x, event.y)
                MotionEvent.ACTION_SCROLL -> wheel(event)
                MotionEvent.ACTION_BUTTON_PRESS, MotionEvent.ACTION_BUTTON_RELEASE ->
                    mouseButtons(event, event.x, event.y)
            }
            return true
        }
        return super.onGenericMotionEvent(event)
    }

    private fun padKey(code: Int, event: KeyEvent): Boolean {
        if (!fromPad(event.source)) return false
        return KeyEvent.isGamepadButton(code) || code == KeyEvent.KEYCODE_BACK ||
            code in KeyEvent.KEYCODE_DPAD_UP..KeyEvent.KEYCODE_DPAD_RIGHT
    }

    override fun onKeyDown(keyCode: Int, event: KeyEvent): Boolean {
        if (padKey(keyCode, event)) {
            if (event.repeatCount == 0) GameBridge.gamepadButton(event.deviceId, keyCode, true)
            return true
        }
        return super.onKeyDown(keyCode, event)
    }

    override fun onKeyUp(keyCode: Int, event: KeyEvent): Boolean {
        if (padKey(keyCode, event)) {
            GameBridge.gamepadButton(event.deviceId, keyCode, false)
            return true
        }
        return super.onKeyUp(keyCode, event)
    }

    private fun pads() {
        val ids = InputDevice.getDeviceIds().filter { id ->
            InputDevice.getDevice(id)?.let { fromPad(it.sources) } == true
        }
        GameBridge.gamepads(ids.toIntArray())
    }

    override fun onInputDeviceAdded(deviceId: Int) = Unit
    override fun onInputDeviceChanged(deviceId: Int) = Unit
    override fun onInputDeviceRemoved(deviceId: Int) = pads()

    companion object {
        const val EXTRA_DIR = "dir"
        const val EXTRA_ARGS = "args"
        const val EXTRA_ENV = "env"
        const val EXTRA_UNDER_CUTOUT = "underCutout"

        private val PAD_AXES = intArrayOf(
            MotionEvent.AXIS_X, MotionEvent.AXIS_Y, MotionEvent.AXIS_Z, MotionEvent.AXIS_RZ,
            MotionEvent.AXIS_HAT_X, MotionEvent.AXIS_HAT_Y,
            MotionEvent.AXIS_LTRIGGER, MotionEvent.AXIS_RTRIGGER,
            MotionEvent.AXIS_GAS, MotionEvent.AXIS_BRAKE,
        )
    }
}
