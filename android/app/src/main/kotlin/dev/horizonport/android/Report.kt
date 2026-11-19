package dev.horizonport.android

import android.app.ActivityManager
import android.content.Context
import android.content.pm.PackageManager
import android.os.Build
import android.system.Os
import android.system.OsConstants
import java.io.File

/**
 * What a player sends when something went wrong on their phone: the device, the settings and
 * the end of every log the game and the system keep. Written into the game's folder too
 * (`android-report.txt`), for whoever copies files over USB.
 */
object Report {
    const val FILE = "android-report.txt"

    fun text(context: Context, lines: Int): String =
        listOf(device(context), settings(context), GameFiles.logTail(context, lines))
            .joinToString("\n\n")

    /** The whole report as a file in the game's folder. */
    fun write(context: Context): File =
        File(GameFiles.dir(context), FILE).apply { writeText(text(context, 1500)) }

    private fun device(context: Context): String {
        val packages = context.packageManager
        val version = packages.getPackageInfo(context.packageName, 0).versionName
        val activity = context.getSystemService(Context.ACTIVITY_SERVICE) as ActivityManager
        val memory = ActivityManager.MemoryInfo().also { activity.getMemoryInfo(it) }
        val metrics = context.resources.displayMetrics
        fun feature(name: String) =
            packages.systemAvailableFeatures.firstOrNull { it.name == name }?.version
        val vulkan = feature(PackageManager.FEATURE_VULKAN_HARDWARE_VERSION)?.let {
            "${it shr 22}.${(it shr 12) and 0x3ff}.${it and 0xfff}"
        } ?: "none reported"
        val chip = if (Build.VERSION.SDK_INT >= 31) "${Build.SOC_MANUFACTURER} ${Build.SOC_MODEL}" else Build.HARDWARE
        val megabyte = 1024 * 1024
        return """
            == Horizon for Android $version ==
            Device: ${Build.MANUFACTURER} ${Build.MODEL} (${Build.DEVICE}), chip $chip, board ${Build.BOARD}
            Android: ${Build.VERSION.RELEASE} (API ${Build.VERSION.SDK_INT}), ABIs ${Build.SUPPORTED_ABIS.joinToString()}
            Memory: ${memory.totalMem / megabyte} MiB, free ${memory.availMem / megabyte} MiB, a large heap of ${activity.largeMemoryClass} MiB, page ${Os.sysconf(OsConstants._SC_PAGESIZE)} bytes
            Screen: ${metrics.widthPixels}x${metrics.heightPixels}, density ${metrics.densityDpi}
            Vulkan: $vulkan, level ${feature(PackageManager.FEATURE_VULKAN_HARDWARE_LEVEL) ?: "-"}, OpenGL ES ${activity.deviceConfigurationInfo.glEsVersion}
            Storage: ${GameFiles.dir(context).usableSpace / megabyte} MiB free in ${GameFiles.dir(context).path}
        """.trimIndent()
    }

    private fun settings(context: Context): String {
        val prefs = Prefs(context)
        val resources = GameFiles.resources(context)
        return """
            == Settings ==
            Resources: ${resources?.path ?: "not found"}
            Prepared by output version ${resources?.let(GameFiles::preparedVersion) ?: "unknown"}; this build reads ${BuildConfig.PREPARED_VERSION}
            Last run: ${GameFiles.lastExit(context)?.replace('\n', ' ') ?: "none"}
            Render distance ${GameFiles.readOption(context, "renderDistance") ?: "-"}, GUI scale ${GameFiles.readOption(context, "guiScale") ?: "-"}
            Touch buttons: size ${prefs.buttonScale}%, opacity ${prefs.buttonOpacity}%, tap uses: ${prefs.tapToUse}
            Under the cutout: ${prefs.underCutout}, input log: ${prefs.logInput}, server: ${if (prefs.server.isEmpty()) "none" else "set"}
        """.trimIndent()
    }
}
