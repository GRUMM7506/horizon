package dev.horizonport.android

import android.content.Context
import android.text.InputType
import android.view.KeyEvent
import android.view.View
import android.view.inputmethod.BaseInputConnection
import android.view.inputmethod.EditorInfo
import android.view.inputmethod.InputConnection

/**
 * The invisible view the on-screen keyboard types into while a text field of the game has the
 * focus. The game keeps its own text and cursor: this only tells it what was typed and what was
 * taken back, by comparing the keyboard's buffer with what it already sent.
 */
class KeyboardSink(context: Context) : View(context) {
    private var connection: Connection? = null

    init {
        isFocusable = true
        isFocusableInTouchMode = true
    }

    /** A new text field: what the keyboard typed before is no longer the game's. */
    fun reset() {
        connection?.reset()
    }

    override fun onCheckIsTextEditor() = true

    override fun onCreateInputConnection(attributes: EditorInfo): InputConnection {
        // Plain text without suggestions: a keyboard that composes words would rewrite text the
        // game already has.
        attributes.inputType = InputType.TYPE_CLASS_TEXT or
            InputType.TYPE_TEXT_FLAG_NO_SUGGESTIONS or
            InputType.TYPE_TEXT_VARIATION_VISIBLE_PASSWORD
        attributes.imeOptions = EditorInfo.IME_FLAG_NO_EXTRACT_UI or
            EditorInfo.IME_FLAG_NO_FULLSCREEN or
            EditorInfo.IME_ACTION_SEND
        return Connection(this).also { connection = it }
    }

    private class Connection(view: View) : BaseInputConnection(view, true) {
        /** What the game was told so far. */
        private var sent = ""

        fun reset() {
            editable?.clear()
            sent = ""
        }

        /** Tells the game how the buffer changed: backspaces to the common start, then the rest. */
        private fun sync() {
            val now = editable?.toString() ?: ""
            var common = sent.commonPrefixWith(now).length
            if (common > 0 && Character.isHighSurrogate(sent[common - 1])) common--
            repeat(sent.codePointCount(common, sent.length)) { GameBridge.backspace() }
            if (now.length > common) GameBridge.text(now.substring(common))
            sent = now
            // The buffer is scratch: start again before it grows without end, between words.
            val text = editable
            if (now.length > 512 && text != null && getComposingSpanStart(text) < 0) reset()
        }

        override fun commitText(text: CharSequence?, newCursorPosition: Int): Boolean =
            super.commitText(text, newCursorPosition).also { sync() }

        override fun setComposingText(text: CharSequence?, newCursorPosition: Int): Boolean =
            super.setComposingText(text, newCursorPosition).also { sync() }

        override fun finishComposingText(): Boolean = super.finishComposingText().also { sync() }

        override fun deleteSurroundingText(beforeLength: Int, afterLength: Int): Boolean {
            if (editable.isNullOrEmpty()) {
                // Nothing of ours left to delete: the game deletes its own text.
                repeat(beforeLength.coerceIn(1, 64)) { GameBridge.backspace() }
                return true
            }
            return super.deleteSurroundingText(beforeLength, afterLength).also { sync() }
        }

        override fun performEditorAction(actionCode: Int): Boolean {
            GameBridge.enter()
            reset()
            return true
        }

        override fun sendKeyEvent(event: KeyEvent): Boolean {
            if (event.action != KeyEvent.ACTION_DOWN) return true
            when (event.keyCode) {
                KeyEvent.KEYCODE_DEL -> {
                    val text = editable
                    if (text.isNullOrEmpty()) {
                        GameBridge.backspace()
                    } else {
                        val end = text.length
                        val start = Character.offsetByCodePoints(text, end, -1)
                        text.delete(start, end)
                        sync()
                    }
                }
                KeyEvent.KEYCODE_ENTER, KeyEvent.KEYCODE_NUMPAD_ENTER -> {
                    GameBridge.enter()
                    reset()
                }
                else -> {
                    val typed = event.unicodeChar
                    if (typed != 0) GameBridge.text(String(Character.toChars(typed)))
                }
            }
            return true
        }
    }
}
