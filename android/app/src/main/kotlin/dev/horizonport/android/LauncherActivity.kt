package dev.horizonport.android

import android.content.Intent
import android.content.pm.PackageManager
import android.net.Uri
import android.os.Bundle
import android.text.format.Formatter
import android.view.View
import android.view.WindowManager
import android.widget.Button
import android.widget.EditText
import android.widget.ProgressBar
import android.widget.SeekBar
import android.widget.TextView
import androidx.activity.result.contract.ActivityResultContracts
import androidx.appcompat.app.AlertDialog
import androidx.appcompat.app.AppCompatActivity
import androidx.appcompat.widget.SwitchCompat
import kotlin.concurrent.thread

/**
 * The launcher: checks that the prepared resources are on the device (and says how to bring
 * them), keeps the player's name, a server to join and a few settings, and starts the game.
 */
class LauncherActivity : AppCompatActivity() {
    private lateinit var prefs: Prefs
    private var import: ZipImport? = null
    private var vulkan = true
    private var vulkanAccepted = false

    private val pickZip = registerForActivityResult(ActivityResultContracts.OpenDocument()) { uri ->
        if (uri != null) importZip(uri)
    }

    override fun onCreate(savedInstanceState: Bundle?) {
        super.onCreate(savedInstanceState)
        setContentView(R.layout.activity_launcher)
        prefs = Prefs(this)
        val version = packageManager.getPackageInfo(packageName, 0).versionName
        findViewById<TextView>(R.id.subtitle).text = getString(R.string.subtitle, version)

        findViewById<EditText>(R.id.name).setText(prefs.name)
        findViewById<EditText>(R.id.server).setText(prefs.server)
        findViewById<SwitchCompat>(R.id.under_cutout).isChecked = prefs.underCutout
        findViewById<SwitchCompat>(R.id.tap_to_use).isChecked = prefs.tapToUse

        // Render distance and GUI scale are the game's own options (options.txt), so what the
        // player changes in the game shows here too.
        val distance = GameFiles.readOption(this, "renderDistance")?.toIntOrNull() ?: DEFAULT_DISTANCE
        slider(R.id.render_distance, R.id.render_distance_label, distance.coerceIn(2, 32) - 2) {
            getString(R.string.render_distance, it + 2)
        }
        val scale = GameFiles.readOption(this, "guiScale")?.toIntOrNull() ?: 0
        slider(R.id.gui_scale, R.id.gui_scale_label, scale.coerceIn(0, 6)) {
            getString(R.string.gui_scale, if (it == 0) getString(R.string.gui_scale_auto) else it.toString())
        }
        slider(R.id.button_scale, R.id.button_scale_label, (prefs.buttonScale.coerceIn(50, 200) - 50) / 5) {
            getString(R.string.button_scale, 50 + it * 5)
        }
        slider(R.id.button_opacity, R.id.button_opacity_label, (prefs.buttonOpacity.coerceIn(10, 100) - 10) / 5) {
            getString(R.string.button_opacity, 10 + it * 5)
        }

        findViewById<Button>(R.id.import_zip).setOnClickListener {
            val running = import
            if (running != null) running.cancelled = true else pickZip.launch(arrayOf("application/zip", "application/octet-stream"))
        }
        findViewById<Button>(R.id.check_again).setOnClickListener { showResources() }
        findViewById<Button>(R.id.play).setOnClickListener { play() }
        findViewById<Button>(R.id.show_log).setOnClickListener { showLog() }
        findViewById<Button>(R.id.share_log).setOnClickListener { shareLog() }
        findViewById<Button>(R.id.dismiss_last_run).setOnClickListener {
            GameFiles.clearLastExit(this)
            showLastRun()
        }

        // Vulkan 1.1 (0x401000), as the system reports it for the device's driver.
        vulkan = packageManager.hasSystemFeature(PackageManager.FEATURE_VULKAN_HARDWARE_VERSION, 0x401000)
        if (!vulkan) {
            findViewById<View>(R.id.vulkan_warning).visibility = View.VISIBLE
            vulkanDialog(startAfter = false)
        }
    }

    override fun onResume() {
        super.onResume()
        showResources()
        showLastRun()
    }

    override fun onPause() {
        super.onPause()
        save()
    }

    private fun slider(id: Int, labelId: Int, start: Int, text: (Int) -> String) {
        val label = findViewById<TextView>(labelId)
        findViewById<SeekBar>(id).apply {
            progress = start
            label.text = text(start)
            setOnSeekBarChangeListener(object : SeekBar.OnSeekBarChangeListener {
                override fun onProgressChanged(bar: SeekBar, value: Int, fromUser: Boolean) {
                    label.text = text(value)
                }
                override fun onStartTrackingTouch(bar: SeekBar) = Unit
                override fun onStopTrackingTouch(bar: SeekBar) = Unit
            })
        }
    }

    private fun value(id: Int) = findViewById<SeekBar>(id).progress

    private fun save() {
        prefs.name = findViewById<EditText>(R.id.name).text.toString().trim()
        prefs.server = findViewById<EditText>(R.id.server).text.toString().trim()
        prefs.buttonScale = 50 + value(R.id.button_scale) * 5
        prefs.buttonOpacity = 10 + value(R.id.button_opacity) * 5
        prefs.underCutout = findViewById<SwitchCompat>(R.id.under_cutout).isChecked
        prefs.tapToUse = findViewById<SwitchCompat>(R.id.tap_to_use).isChecked
    }

    private fun showResources() {
        val found = GameFiles.resources(this)
        val status = findViewById<TextView>(R.id.resources_status)
        val help = findViewById<TextView>(R.id.resources_help)
        if (found != null) {
            status.text = getString(R.string.resources_found, found.path)
            help.visibility = View.GONE
        } else {
            status.text = getString(R.string.resources_missing)
            help.text = getString(R.string.resources_help, GameFiles.dir(this).path)
            help.visibility = View.VISIBLE
        }
        findViewById<Button>(R.id.play).isEnabled = found != null && import == null
    }

    private fun showLastRun() {
        val exit = GameFiles.lastExit(this)
        val box = findViewById<View>(R.id.last_run)
        val text = findViewById<TextView>(R.id.last_run_text)
        when {
            exit == null || exit == "ok" -> box.visibility = View.GONE
            exit.startsWith("error") -> {
                text.text = getString(R.string.last_run_error, exit.substringAfter('\n', "").trim())
                box.visibility = View.VISIBLE
            }
            // "running" (the process died without saying why) or "panic".
            else -> {
                text.text = getString(R.string.last_run_crash)
                box.visibility = View.VISIBLE
            }
        }
    }

    private fun showLog() {
        val log = GameFiles.logTail(this).ifEmpty { getString(R.string.log_empty) }
        val view = TextView(this).apply {
            text = log
            textSize = 11f
            setTextIsSelectable(true)
            setPadding(32, 16, 32, 16)
        }
        AlertDialog.Builder(this)
            .setTitle(R.string.log_title)
            .setView(android.widget.ScrollView(this).apply { addView(view) })
            .setPositiveButton(R.string.close, null)
            .show()
    }

    private fun shareLog() {
        val log = GameFiles.logTail(this, 400).ifEmpty { getString(R.string.log_empty) }
        val send = Intent(Intent.ACTION_SEND)
            .setType("text/plain")
            .putExtra(Intent.EXTRA_SUBJECT, "Horizon Android log")
            .putExtra(Intent.EXTRA_TEXT, log.takeLast(90_000))
        startActivity(Intent.createChooser(send, getString(R.string.share_log)))
    }

    private fun vulkanDialog(startAfter: Boolean) {
        AlertDialog.Builder(this)
            .setTitle(R.string.vulkan_title)
            .setMessage(R.string.vulkan_message)
            .setNegativeButton(R.string.close, null)
            .setPositiveButton(R.string.try_anyway) { _, _ ->
                vulkanAccepted = true
                if (startAfter) play()
            }
            .show()
    }

    private fun importZip(uri: Uri) {
        val bar = findViewById<ProgressBar>(R.id.import_progress)
        val status = findViewById<TextView>(R.id.import_status)
        val button = findViewById<Button>(R.id.import_zip)
        bar.visibility = View.VISIBLE
        bar.isIndeterminate = true
        status.visibility = View.VISIBLE
        status.text = getString(R.string.importing, "…")
        button.setText(R.string.cancel)
        window.addFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
        val job = ZipImport(contentResolver, uri, GameFiles.dir(this)) { done, total ->
            runOnUiThread {
                bar.isIndeterminate = total <= 0
                if (total > 0) bar.progress = (done * 1000 / total).toInt()
                status.text = getString(R.string.importing, Formatter.formatShortFileSize(this, done))
            }
        }
        import = job
        showResources()
        thread(name = "zip import") {
            val result = runCatching { job.run() }
            runOnUiThread {
                import = null
                bar.visibility = View.GONE
                button.setText(R.string.import_zip)
                window.clearFlags(WindowManager.LayoutParams.FLAG_KEEP_SCREEN_ON)
                status.text = result.fold(
                    { getString(R.string.import_done, it) },
                    { getString(R.string.import_failed, it.message ?: it.toString()) },
                )
                showResources()
            }
        }
    }

    private fun play() {
        val resources = GameFiles.resources(this) ?: return showResources()
        if (!vulkan && !vulkanAccepted) return vulkanDialog(startAfter = true)
        save()
        val nameField = findViewById<EditText>(R.id.name)
        if (!Prefs.validName(prefs.name)) {
            nameField.error = getString(R.string.name_invalid)
            nameField.requestFocus()
            return
        }
        GameFiles.writeOptions(
            this,
            mapOf(
                "renderDistance" to (value(R.id.render_distance) + 2).toString(),
                "guiScale" to value(R.id.gui_scale).toString(),
            ),
        )
        GameFiles.clearLastExit(this)
        val dir = GameFiles.dir(this)
        val args = mutableListOf(
            "--game-dir", dir.path,
            "--home", dir.path,
            "--resources", resources.path,
            "--name", prefs.name,
        )
        if (prefs.server.isNotEmpty()) args += listOf("--server", prefs.server)
        val env = arrayOf(
            "HORIZON_TOUCH_SCALE=${prefs.buttonScale / 100f}",
            "HORIZON_TOUCH_OPACITY=${prefs.buttonOpacity / 100f}",
            "HORIZON_TOUCH_TAP_USES=${if (prefs.tapToUse) 1 else 0}",
        )
        startActivity(
            Intent(this, HorizonActivity::class.java)
                .putExtra(HorizonActivity.EXTRA_DIR, dir.path)
                .putExtra(HorizonActivity.EXTRA_ARGS, args.toTypedArray())
                .putExtra(HorizonActivity.EXTRA_ENV, env)
                .putExtra(HorizonActivity.EXTRA_UNDER_CUTOUT, prefs.underCutout),
        )
    }

    companion object {
        /** A phone's default: the game's own is 12. */
        private const val DEFAULT_DISTANCE = 8
    }
}
