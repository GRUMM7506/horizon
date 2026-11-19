package dev.horizonport.android

import android.content.ContentResolver
import android.net.Uri
import android.provider.OpenableColumns
import java.io.File
import java.io.FilterInputStream
import java.io.InputStream
import java.util.zip.ZipInputStream

/**
 * Unpacks a zip of the prepared `.local` folder, picked with the system's file chooser, into the
 * game's folder. Entries that would leave the folder are refused.
 */
class ZipImport(
    private val resolver: ContentResolver,
    private val source: Uri,
    private val target: File,
    private val progress: (done: Long, total: Long) -> Unit,
) {
    @Volatile var cancelled = false

    /** The number of files written. Throws on a bad archive or a full disk. */
    fun run(): Int {
        val total = size()
        val root = target.canonicalFile
        var files = 0
        val input = resolver.openInputStream(source) ?: error("cannot open the archive")
        Counting(input).use { counted ->
            ZipInputStream(counted.buffered(1 shl 16)).use { zip ->
                val buffer = ByteArray(1 shl 16)
                var last = 0L
                while (true) {
                    val entry = zip.nextEntry ?: break
                    if (cancelled) error("cancelled")
                    val out = File(root, entry.name).canonicalFile
                    if (out != root && !out.path.startsWith(root.path + File.separator)) {
                        error("the archive holds a path outside the game folder: ${entry.name}")
                    }
                    if (entry.isDirectory) {
                        out.mkdirs()
                        continue
                    }
                    out.parentFile?.mkdirs()
                    out.outputStream().use { file ->
                        while (true) {
                            val read = zip.read(buffer)
                            if (read < 0) break
                            if (cancelled) error("cancelled")
                            file.write(buffer, 0, read)
                            if (counted.count - last > (1 shl 20)) {
                                last = counted.count
                                progress(last, total)
                            }
                        }
                    }
                    files++
                }
            }
        }
        return files
    }

    private fun size(): Long =
        resolver.query(source, arrayOf(OpenableColumns.SIZE), null, null, null)?.use { cursor ->
            if (cursor.moveToFirst() && !cursor.isNull(0)) cursor.getLong(0) else -1L
        } ?: -1L

    private class Counting(input: InputStream) : FilterInputStream(input) {
        @Volatile var count = 0L
        override fun read(): Int = super.read().also { if (it >= 0) count++ }
        override fun read(b: ByteArray, off: Int, len: Int): Int =
            super.read(b, off, len).also { if (it > 0) count += it }
    }
}
