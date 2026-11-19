package dev.horizonport.android

import android.content.Context
import java.io.File

/**
 * The game's folder on the device: `Android/data/<package>/files/horizon`. The player copies the
 * prepared `.local` folder from a computer into it; saves, options and logs live there too.
 */
object GameFiles {
    /** The file `horizon-prepare` writes last, at the root of the prepared resources. */
    private const val MARKER = "horizon-resources.json"

    /** The game's own folders, which hold no prepared resources. */
    private val SKIPPED = setOf("saves", "logs", "crash-reports", "screenshots", "resourcepacks", ".horizon-cache")

    fun dir(context: Context): File {
        val base = context.getExternalFilesDir(null) ?: context.filesDir
        return File(base, "horizon").apply { mkdirs() }
    }

    /**
     * The prepared resources: `.local/horizon-client/<version>` as the computer has it, or the
     * same folder copied without one of its parents.
     */
    fun resources(context: Context): File? {
        val root = dir(context)
        // Where it belongs first; then wherever a copy or an archive with one folder more (or
        // less) around it put it. Worlds and logs are not looked into.
        File(root, ".local/horizon-client").listFiles()
            ?.filter { File(it, MARKER).isFile }
            ?.maxByOrNull { it.name }
            ?.let { return it }
        return find(root, 4)
    }

    private fun find(folder: File, depth: Int): File? {
        if (File(folder, MARKER).isFile) return folder
        if (depth == 0) return null
        val inside = folder.listFiles()?.filter { it.isDirectory && it.name !in SKIPPED } ?: return null
        return inside.sortedByDescending { it.name }.firstNotNullOfOrNull { find(it, depth - 1) }
    }

    fun options(context: Context) = File(dir(context), "options.txt")

    /** How the last run ended, as the game's library wrote it; null before the first run. */
    fun lastExit(context: Context): String? =
        File(dir(context), "last-exit.txt").takeIf { it.isFile }?.readText()?.trim()

    fun clearLastExit(context: Context) {
        File(dir(context), "last-exit.txt").delete()
    }

    /** The end of the newest log, for a run that ended badly. */
    fun logTail(context: Context, lines: Int = 60): String {
        val logs = File(dir(context), "logs")
        val parts = listOf("latest.log", "android-output.log").mapNotNull { name ->
            val file = File(logs, name)
            if (!file.isFile) return@mapNotNull null
            val tail = file.useLines { it.toList().takeLast(lines) }.joinToString("\n")
            "== $name ==\n$tail"
        }
        return parts.joinToString("\n\n")
    }

    /** `key:value` lines of options.txt, the others untouched. */
    fun readOption(context: Context, key: String): String? =
        options(context).takeIf { it.isFile }?.useLines { lines ->
            lines.firstOrNull { it.startsWith("$key:") }?.substringAfter(':')
        }

    fun writeOptions(context: Context, values: Map<String, String>) {
        val file = options(context)
        val left = values.toMutableMap()
        val lines = if (file.isFile) file.readLines() else emptyList()
        val out = lines.map { line ->
            val key = line.substringBefore(':', "")
            left.remove(key)?.let { "$key:$it" } ?: line
        } + left.map { (key, value) -> "$key:$value" }
        file.writeText(out.joinToString("\n", postfix = "\n"))
    }
}
