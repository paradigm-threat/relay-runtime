package com.clevertree.relayruntime.demo

import android.content.Context
import android.util.AttributeSet
import android.util.Log
import android.view.ViewGroup
import android.widget.FrameLayout
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.clevertree.relayruntime.RelayRuntime
import org.json.JSONArray
import org.json.JSONObject
import java.io.InputStreamReader

/**
 * Core relay-runtime component: renders local or remote hooks.
 * Local: hookPath (e.g. "hooks/demo.jsx") — loads from assets.
 * Remote: host + hookPath — fetches from host (Phase F, stub for now).
 * Used as main root by MainActivity; can be nested for remote peer content.
 */
class HookRenderer @JvmOverloads constructor(
    context: Context,
    attrs: AttributeSet? = null,
    defStyleAttr: Int = 0
) : FrameLayout(context, attrs, defStyleAttr) {

    companion object {
        private const val TAG = "RelayDemo"
        private const val HOOKS_ASSET_DIR = "hooks"
    }

    private val scrollView: ScrollView
    private val container: FrameLayout
    private val renderer: VNodeRenderer

    private fun isDarkTheme(): Boolean {
        val themeName = RelayRuntime.getCurrentTheme()
        val prefersDark = (context.resources.configuration.uiMode and
            android.content.res.Configuration.UI_MODE_NIGHT_MASK) ==
            android.content.res.Configuration.UI_MODE_NIGHT_YES
        return themeName == "dark" || (themeName == "default" && prefersDark)
    }

    private fun applyThemeToChrome() {
        val dark = isDarkTheme()
        val bgColor = if (dark) android.graphics.Color.parseColor("#111827") else android.graphics.Color.WHITE
        val containerColor = if (dark) android.graphics.Color.parseColor("#111827") else android.graphics.Color.parseColor("#f5f5f5")
        scrollView.setBackgroundColor(bgColor)
        container.setBackgroundColor(containerColor)
    }

    /** Local: path in module map. Default: hooks/app.jsx (single entry). */
    var hookPath: String = "hooks/app.jsx"
        set(value) {
            field = value
            runRender()
        }

    /** Remote: base URL. When set, fetches hook from host (stub: not yet implemented). */
    var host: String? = null
        set(value) {
            field = value
            if (value != null) runRender()
        }

    var onError: ((String) -> Unit)? = null
    var onReady: (() -> Unit)? = null

    init {
        scrollView = ScrollView(context).apply {
            layoutParams = LayoutParams(
                LayoutParams.MATCH_PARENT,
                LayoutParams.MATCH_PARENT
            )
            isFillViewport = true
        }
        container = FrameLayout(context).apply {
            layoutParams = LayoutParams(
                LayoutParams.MATCH_PARENT,
                LayoutParams.WRAP_CONTENT
            )
            setPadding(32, 32, 32, 32)
        }
        scrollView.addView(container)
        addView(scrollView)
        renderer = VNodeRenderer(context)
        applyThemeToChrome()
    }

    /** Call after view is attached to load and render the hook. */
    fun runRender() {
        if (host != null) {
            runRenderRemote()
            return
        }
        runRenderLocal()
    }

    private fun runRenderLocal() {
        Log.d(TAG, "HookRenderer.runRender: hookPath=$hookPath")
        try {
            val loaded = loadHooksFromAssets()
            if (loaded == null) {
                showError("No hooks found in assets/$HOOKS_ASSET_DIR")
                return
            }
            val (moduleMap, hooksManifest) = loaded
            val entrySource = moduleMap.optString(hookPath)
            if (entrySource.isEmpty()) {
                showError("Entry hook $hookPath not found in assets")
                return
            }
            val themeYaml = loadThemeYaml()
            val themeName = RelayRuntime.getCurrentTheme()
            val prefersDark = (context.resources.configuration.uiMode and
                android.content.res.Configuration.UI_MODE_NIGHT_MASK) ==
                android.content.res.Configuration.UI_MODE_NIGHT_YES
            val vnodeJson = RelayRuntime.runHookWithHost(
                source = entrySource,
                moduleMapJson = moduleMap.toString(),
                themeYaml = themeYaml,
                hooksManifestJson = hooksManifest.toString(),
                file = hookPath,
                themeName = themeName,
                prefersDark = prefersDark
            )
            Log.i(TAG, "HookRenderer: hook OK")
            applyThemeToChrome()
            val ok = renderer.render(vnodeJson, container) { runRender() }
            if (!ok) {
                showError("Render returned false")
            } else {
                onReady?.invoke()
            }
        } catch (e: Exception) {
            Log.e(TAG, "HookRenderer: parse/runtime error", e)
            showError(e.message ?: e.toString(), e.stackTraceToString())
        }
    }

    private fun runRenderRemote() {
        // TODO Phase F: fetch(host OPTIONS) → hookPath, fetch(host + path) → source, run
        showError("Remote hooks not yet implemented (host=${host})")
    }

    private fun loadThemeYaml(): String {
        return try {
            context.assets.open("theme.yaml").use { it.bufferedReader().readText() }
        } catch (_: Exception) {
            ""
        }
    }

    private fun loadHooksFromAssets(): Pair<JSONObject, JSONObject>? {
        return try {
            val moduleMap = JSONObject()
            val hooksArray = JSONArray()
            context.assets.list(HOOKS_ASSET_DIR)?.filter { it.endsWith(".jsx") }?.sorted()?.forEach { filename ->
                val path = "$HOOKS_ASSET_DIR/$filename"
                val source = context.assets.open(path).use { stream ->
                    InputStreamReader(stream).readText()
                }
                moduleMap.put(path, source)
                val name = filename.removeSuffix(".jsx").replaceFirstChar { it.uppercase() }
                hooksArray.put(JSONObject().apply {
                    put("path", path)
                    put("name", name)
                })
            }
            if (moduleMap.length() == 0) {
                Log.e(TAG, "loadHooksFromAssets: no .jsx files in $HOOKS_ASSET_DIR")
                return null
            }
            val hooksManifest = JSONObject().put("hooks", hooksArray)
            Pair(moduleMap, hooksManifest)
        } catch (e: Exception) {
            Log.e(TAG, "loadHooksFromAssets failed", e)
            null
        }
    }

    private fun showError(msg: String?, stackTrace: String? = null) {
        container.removeAllViews()
        val (title, detail) = parseRelayError(msg)
        val layout = LinearLayout(context).apply {
            orientation = LinearLayout.VERTICAL
            setBackgroundColor(android.graphics.Color.parseColor("#FEF2F2"))
            setPadding(24, 24, 24, 24)
        }
        layout.addView(TextView(context).apply {
            text = "⚠ $title"
            setTextColor(android.graphics.Color.parseColor("#B91C1C"))
            textSize = 16f
            setTypeface(null, android.graphics.Typeface.BOLD)
            setPadding(0, 0, 0, 8)
        })
        layout.addView(TextView(context).apply {
            text = detail
            setTextColor(android.graphics.Color.parseColor("#7F1D1D"))
            textSize = 14f
            setPadding(0, 0, 0, 16)
        })
        if (!stackTrace.isNullOrBlank()) {
            layout.addView(TextView(context).apply {
                text = "Details:\n$stackTrace"
                setTextColor(android.graphics.Color.GRAY)
                textSize = 11f
                setPadding(0, 8, 0, 0)
            })
        }
        layout.layoutParams = LayoutParams(
            LayoutParams.MATCH_PARENT,
            LayoutParams.WRAP_CONTENT
        )
        container.addView(layout)
        onError?.invoke(msg ?: "")
    }

    private fun parseRelayError(msg: String?): Pair<String, String> {
        if (msg == null) return "Error" to ""
        val jsonPrefix = "RELAY_ERROR_JSON:"
        if (msg.startsWith(jsonPrefix)) {
            return try {
                val json = org.json.JSONObject(msg.removePrefix(jsonPrefix))
                val type = json.optString("type", "error")
                val message = json.optString("message", "")
                val file = json.optString("file", "").takeIf { it.isNotEmpty() }
                val line = json.optInt("line", 0).takeIf { it > 0 }
                val col = json.optInt("column", 0).takeIf { it > 0 }
                val sourceLine = json.optString("source_line", "").takeIf { it.isNotEmpty() }
                val loc = when {
                    file != null && line != null -> "$file:$line" + (col?.let { ":$it" } ?: "")
                    line != null -> "line $line" + (col?.let { ":$it" } ?: "")
                    else -> ""
                }
                val title = when (type) {
                    "parse" -> "Parse Error"
                    "runtime" -> "Runtime Error"
                    else -> "Error"
                }
                val detail = buildString {
                    append(message)
                    if (loc.isNotEmpty()) append("\n  at $loc")
                    if (sourceLine != null) append("\n  $sourceLine")
                }
                title to detail
            } catch (_: Exception) {
                "Error" to msg
            }
        }
        return "Error" to msg
    }
}
