package dev.horizonport.android

import android.content.Context
import android.content.SharedPreferences

/** What the launcher remembers between runs. */
class Prefs(context: Context) {
    private val store: SharedPreferences =
        context.getSharedPreferences("launcher", Context.MODE_PRIVATE)

    var name: String
        get() = store.getString("name", "Player") ?: "Player"
        set(value) = store.edit().putString("name", value).apply()

    var server: String
        get() = store.getString("server", "") ?: ""
        set(value) = store.edit().putString("server", value).apply()

    /** Touch button size, percent of the default. */
    var buttonScale: Int
        get() = store.getInt("buttonScale", 100)
        set(value) = store.edit().putInt("buttonScale", value).apply()

    /** Touch button opacity, percent. */
    var buttonOpacity: Int
        get() = store.getInt("buttonOpacity", 65)
        set(value) = store.edit().putInt("buttonOpacity", value).apply()

    /** Draw under the display cutout instead of leaving its side black. */
    var underCutout: Boolean
        get() = store.getBoolean("underCutout", false)
        set(value) = store.edit().putBoolean("underCutout", value).apply()

    /** Tap the world to use and hold to mine (as Bedrock does), instead of tap to attack. */
    var tapToUse: Boolean
        get() = store.getBoolean("tapToUse", false)
        set(value) = store.edit().putBoolean("tapToUse", value).apply()

    /** Every touch and key goes to the game's log (`HORIZON_ANDROID_LOG_INPUT`). */
    var logInput: Boolean
        get() = store.getBoolean("logInput", false)
        set(value) = store.edit().putBoolean("logInput", value).apply()

    companion object {
        /** `Player` names vanilla accepts offline: 3 to 16 of these characters. */
        fun validName(name: String) = Regex("[A-Za-z0-9_]{3,16}").matches(name)
    }
}
