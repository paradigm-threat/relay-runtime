package com.clevertree.relayruntime.demo

import android.content.Context
import android.graphics.Color
import android.graphics.drawable.GradientDrawable
import android.util.Log
import android.util.TypedValue
import android.view.Gravity
import android.view.View
import android.view.ViewGroup
import android.widget.Button
import android.widget.LinearLayout
import android.widget.ScrollView
import android.widget.TextView
import com.clevertree.relayruntime.RelayRuntime
import org.json.JSONObject

/**
 * Renders VNode JSON from relay-runtime into Android Views.
 * Supports: div, span, p, h1–h6, button, fragment; Text nodes; onClick→invokeHandler.
 *
 * NOTE: Style translation (className→style, units, theme vars) is done in Rust
 * (style_resolve.rs + themed-styler) before this receives the JSON. This renderer
 * only APPLIES the pre-resolved style props; it does not interpret className.
 */
class VNodeRenderer(private val context: Context) {

    companion object {
        private const val TAG = "RelayDemo"
    }

    /**
     * @param onRenderRequested Call when invokeHandler triggers setState; host should re-run hook and call render again.
     */
    fun render(vnodeJson: String, container: ViewGroup, onRenderRequested: () -> Unit): Boolean {
        return try {
            Log.d(TAG, "VNodeRenderer.render: input len=${vnodeJson.length}, preview=${vnodeJson.take(150)}...")
            container.removeAllViews()
            val root = parseAndBuild(vnodeJson, onRenderRequested)
            if (root == null) {
                Log.e(TAG, "VNodeRenderer.render: parseAndBuild returned null (unknown VNode format)")
                return false
            }
            root.layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
            // Root sizes to content; no min height (was causing layout issues)
            container.addView(root)
            Log.d(TAG, "VNodeRenderer.render: OK")
            true
        } catch (e: org.json.JSONException) {
            Log.e(TAG, "VNodeRenderer.render: JSON parse error", e)
            Log.e(TAG, "VNodeRenderer.render: input was: ${vnodeJson.take(500)}")
            showRenderError(container, "Parse error: ${e.message}")
            false
        } catch (e: Exception) {
            Log.e(TAG, "VNodeRenderer.render: error", e)
            showRenderError(container, "Render error: ${e.message}")
            false
        }
    }

    private fun showRenderError(container: ViewGroup, msg: String) {
        container.removeAllViews()
        container.addView(TextView(context).apply {
            text = msg
            setTextColor(Color.RED)
            setPadding(48, 48, 48, 48)
        })
    }

    private fun parseAndBuild(json: String, onRenderRequested: () -> Unit): View? {
        return try {
            val raw = org.json.JSONObject(json)
            when {
                raw.has("Element") -> buildElement(raw.getJSONObject("Element"), onRenderRequested)
                raw.has("Text") -> buildText(raw.getString("Text"))
                raw.has("Empty") -> {
                    // CleverScript: hook normalized to empty vnode (e.g. wrong import path, null UI)
                    Log.w(TAG, "VNodeRenderer.parseAndBuild: Empty vnode — hook produced no element tree")
                    TextView(context).apply {
                        text = "Empty render (hook returned no UI). Check dynamic import paths match module map keys (e.g. hooks/home.jsx)."
                        setTextColor(Color.GRAY)
                        setPadding(dp(16), dp(16), dp(16), dp(16))
                    }
                }
                else -> {
                    Log.w(TAG, "VNodeRenderer.parseAndBuild: unknown format keys=${raw.keys().asSequence().toList()}")
                    null
                }
            }
        } catch (e: org.json.JSONException) {
            Log.e(TAG, "VNodeRenderer.parseAndBuild: JSON error on '$json'", e)
            throw e
        }
    }

    private fun buildElement(el: JSONObject, onRenderRequested: () -> Unit): View {
        val tag = el.optString("tag", "div")
        val props = el.optJSONObject("props") ?: JSONObject()
        val children = el.optJSONArray("children")

        when (tag.lowercase()) {
            "relay-hook-renderer" -> return buildNestedHookRenderer(props, onRenderRequested)
            else -> { /* fall through */ }
        }
        val view = when (tag.lowercase()) {
            "button" -> Button(context)
            "h1", "h2", "h3", "h4", "h5", "h6" -> TextView(context).apply {
                textSize = headingSize(tag)
                setTextColor(Color.BLACK)
            }
            "p", "span", "text" -> TextView(context).apply { setTextColor(Color.BLACK) }
            "div", "view", "section", "header", "footer", "main", "fragment" -> {
                LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
            }
            "scroll", "scrollview" -> ScrollView(context).apply {
                addView(LinearLayout(context).apply { orientation = LinearLayout.VERTICAL })
            }
            else -> LinearLayout(context).apply { orientation = LinearLayout.VERTICAL }
        }

        applyProps(view, tag, props, onRenderRequested)

        if (children != null) {
            if (view is ViewGroup) {
                val content = when (view) {
                    is ScrollView -> (view.getChildAt(0) as? LinearLayout) ?: view
                    else -> view
                }
                val parentStyle = styleFromProps(props)
                val isRow = (view is LinearLayout) && (view.orientation == LinearLayout.HORIZONTAL)
                val spacingPx = when {
                    isRow -> styleInt(parentStyle, "spaceX") ?: styleInt(parentStyle, "gap") ?: 0
                    else -> styleInt(parentStyle, "spaceY") ?: styleInt(parentStyle, "gap") ?: 0
                }
                for (i in 0 until children.length()) {
                    val childJson = children.get(i).toString()
                    val childEl = try { org.json.JSONObject(childJson) } catch (_: Exception) { null }
                    val childView = try {
                        parseAndBuild(childJson, onRenderRequested)
                    } catch (e: Exception) {
                        Log.e(TAG, "VNodeRenderer.buildElement: child[$i] parse failed: $childJson", e)
                        null
                    }
                    if (childView != null) {
                        val lp = layoutParamsFor(content, childView, childEl?.optJSONObject("props"))
                        if (lp is ViewGroup.MarginLayoutParams && i > 0 && spacingPx > 0) {
                            if (isRow) lp.leftMargin = spacingPx else lp.topMargin = spacingPx
                        }
                        content.addView(childView, lp)
                    }
                }
            } else if (view is TextView) {
                // TextView/Button can't hold ViewGroup children; concatenate Text nodes as content
                val sb = StringBuilder()
                for (i in 0 until children.length()) {
                    val child = org.json.JSONObject(children.get(i).toString())
                    if (child.has("Text")) sb.append(child.getString("Text"))
                }
                val text = sb.toString()
                view.text = text
                if (view is Button && text.isNotEmpty()) view.setAllCaps(false)
            }
        }
        return view
    }

    private fun layoutParamsFor(parent: ViewGroup, child: View, childProps: JSONObject?): ViewGroup.LayoutParams {
        val style = styleFromProps(childProps)
        val width = resolveDimension(style, "width", ViewGroup.LayoutParams.MATCH_PARENT)
        val height = resolveDimension(style, "height", ViewGroup.LayoutParams.WRAP_CONTENT)
        val params = when (parent) {
            is LinearLayout -> LinearLayout.LayoutParams(width, height)
            is ScrollView -> ViewGroup.LayoutParams(width, height)
            else -> ViewGroup.LayoutParams(width, height)
        }
        if (params is ViewGroup.MarginLayoutParams && style != null) {
            val m = styleInt(style, "margin")
            params.topMargin = styleInt(style, "marginTop") ?: m ?: 0
            params.bottomMargin = styleInt(style, "marginBottom") ?: m ?: 0
            params.leftMargin = styleInt(style, "marginLeft") ?: m ?: 0
            params.rightMargin = styleInt(style, "marginRight") ?: m ?: 0
        }
        if (params is LinearLayout.LayoutParams && style != null) {
            val weight = styleDouble(style, "flex") ?: styleDouble(style, "flexGrow") ?: 0.0
            params.weight = weight.toFloat()
            if (weight > 0 && parent is LinearLayout) {
                if (parent.orientation == LinearLayout.HORIZONTAL) {
                    params.width = 0
                } else {
                    params.height = 0
                }
            }
            val layoutGravity = styleValue(style, "androidLayoutGravity")
            if (layoutGravity.isNotEmpty()) params.gravity = parseGravity(layoutGravity)
        }
        return params
    }

    private fun resolveDimension(style: JSONObject?, key: String, default: Int): Int {
        if (style == null) return default
        val v = style.opt(key) ?: return default
        val s = when (v) {
            is String -> v
            is JSONObject -> v.optString("value", v.optString("String", ""))
            else -> v.toString()
        }
        return when (s.lowercase()) {
            "match_parent", "100%" -> ViewGroup.LayoutParams.MATCH_PARENT
            "wrap_content", "fit-content", "auto" -> ViewGroup.LayoutParams.WRAP_CONTENT
            else -> styleInt(style, key) ?: default
        }
    }

    private fun parseGravity(s: String): Int {
        var g = Gravity.NO_GRAVITY
        for (part in s.split("|").map { it.trim().lowercase() }) {
            g = g or when (part) {
                "center" -> Gravity.CENTER
                "center_horizontal" -> Gravity.CENTER_HORIZONTAL
                "center_vertical" -> Gravity.CENTER_VERTICAL
                "top" -> Gravity.TOP
                "bottom" -> Gravity.BOTTOM
                "start" -> Gravity.START
                "end" -> Gravity.END
                else -> Gravity.NO_GRAVITY
            }
        }
        return g
    }

    private fun headingSize(tag: String): Float = when (tag) {
        "h1" -> 28f; "h2" -> 24f; "h3" -> 20f; "h4" -> 18f; "h5" -> 16f; "h6" -> 14f
        else -> 16f
    }

    private fun buildText(text: String): TextView = TextView(context).apply {
        this.text = text
        setTextColor(Color.BLACK)
    }

    private fun buildNestedHookRenderer(props: JSONObject, onRenderRequested: () -> Unit): View {
        val host = propString(props, "host") ?: return TextView(context).apply {
            text = "relay-hook-renderer: host prop required"
            setTextColor(Color.RED)
        }
        val hookPath = propString(props, "hookPath") ?: "/hooks/client/get.jsx"
        return HookRenderer(context).apply {
            this.host = host
            this.hookPath = hookPath
            layoutParams = ViewGroup.LayoutParams(
                ViewGroup.LayoutParams.MATCH_PARENT,
                ViewGroup.LayoutParams.WRAP_CONTENT
            )
            runRender()
        }
    }

    private fun applyProps(view: View, tag: String, props: JSONObject, onRenderRequested: () -> Unit) {
        // Apply pre-resolved style prop only. No className parsing (handled in relay-runtime core).
        val styleObj = props.optJSONObject("style") ?: (props.opt("style") as? JSONObject)
        // Style values from relay-runtime are in px (themed-styler converts dp→px by density)
        val style = styleObj?.optJSONObject("value") ?: styleObj
        style?.let { s ->
            val padAll = styleInt(s, "padding")
            var pl = padAll ?: 0
            var pt = padAll ?: 0
            var pr = padAll ?: 0
            var pb = padAll ?: 0
            styleInt(s, "paddingLeft")?.let { pl = it }
            styleInt(s, "paddingTop")?.let { pt = it }
            styleInt(s, "paddingRight")?.let { pr = it }
            styleInt(s, "paddingBottom")?.let { pb = it }
            if (padAll != null || s.has("paddingLeft") || s.has("paddingTop") || s.has("paddingRight") || s.has("paddingBottom")) {
                view.setPadding(pl, pt, pr, pb)
            }
            var borderRadius = 0f
            var borderWidth = 0
            var borderColor: Int? = null
            var backgroundColor: Int? = null
            for (key in s.keys()) {
                when (key) {
                    "color" -> {
                        val v = styleValue(s, key)
                        if (v.isNotEmpty() && v.startsWith("#") && view is TextView) {
                            try { view.setTextColor(Color.parseColor(v)) } catch (_: Exception) {}
                        }
                    }
                    "backgroundColor" -> {
                        val v = styleValue(s, key)
                        if (v.isNotEmpty() && v.startsWith("#")) {
                            try { backgroundColor = Color.parseColor(v) } catch (_: Exception) {}
                        }
                    }
                    "fontSize" -> {
                        val size = (styleDouble(s, key) ?: 16.0).toFloat()
                        if (view is TextView) view.setTextSize(TypedValue.COMPLEX_UNIT_SP, size)
                    }
                    "fontWeight" -> {
                        val w = styleValue(s, key).lowercase()
                        if ((view is TextView) && (w == "bold" || w == "600" || w == "700")) {
                            view.setTypeface(null, android.graphics.Typeface.BOLD)
                        }
                    }
                    "androidTypefaceStyle" -> {
                        val tf = when (styleValue(s, key).lowercase()) {
                            "bold" -> android.graphics.Typeface.BOLD
                            "italic" -> android.graphics.Typeface.ITALIC
                            "bold_italic" -> android.graphics.Typeface.BOLD_ITALIC
                            else -> android.graphics.Typeface.NORMAL
                        }
                        if (view is TextView) view.setTypeface(null, tf)
                    }
                    "androidTextGravity" -> {
                        if (view is TextView) view.gravity = parseGravity(styleValue(s, key))
                    }
                    "androidGravity" -> {
                        if (view is LinearLayout) view.gravity = parseGravity(styleValue(s, key))
                    }
                    "flexDirection" -> {
                        if (view is LinearLayout) {
                            val fd = styleValue(s, key).lowercase()
                            view.orientation = if (fd == "row") LinearLayout.HORIZONTAL else LinearLayout.VERTICAL
                        }
                    }
                    "display" -> {
                        val d = styleValue(s, key).lowercase()
                        view.visibility = if (d == "none" || d == "hidden") View.GONE else View.VISIBLE
                    }
                    "elevation" -> view.elevation = (styleDouble(s, key) ?: 0.0).toFloat()
                    "borderRadius" -> borderRadius = (styleDouble(s, key) ?: 0.0).toFloat()
                    "borderWidth" -> borderWidth = styleInt(s, key) ?: 0
                    "borderColor" -> {
                        val v = styleValue(s, key)
                        if (v.startsWith("#")) try { borderColor = Color.parseColor(v) } catch (_: Exception) {}
                    }
                    "padding", "paddingTop", "paddingBottom", "paddingLeft", "paddingRight" -> { /* handled above */ }
                    else -> { /* skip */ }
                }
            }
            if (borderRadius > 0 || borderWidth > 0 || backgroundColor != null) {
                val shape = GradientDrawable()
                if (borderRadius > 0) {
                    shape.cornerRadius = borderRadius
                    view.clipToOutline = true
                }
                if (backgroundColor != null) shape.setColor(backgroundColor!!)
                if (borderWidth > 0) shape.setStroke(borderWidth, borderColor ?: Color.TRANSPARENT)
                view.background = shape
            } else if (backgroundColor != null) {
                view.setBackgroundColor(backgroundColor!!)
            }
        }

        // children text for container (div with single text child)
        propString(props, "text")?.let { text ->
            if (view is TextView) view.text = text
            else if (view is ViewGroup && view.childCount == 0) {
                view.addView(TextView(context).apply { this.text = text })
            }
        }

        // onClick -> HandlerId (serialized as {"type":"HandlerId","value":id})
        (props.optJSONObject("onClick") ?: (props.opt("onClick") as? JSONObject))?.let { handler ->
            val id = handler.optInt("value", 0)
            if (id > 0) {
                view.isClickable = true
                view.setOnClickListener {
                    try {
                        Log.d(TAG, "VNodeRenderer: onClick handlerId=$id")
                        RelayRuntime.invokeHandler(id)
                        val needRender = RelayRuntime.consumeRenderRequested()
                        Log.d(TAG, "VNodeRenderer: consumeRenderRequested=$needRender")
                        if (needRender) onRenderRequested()
                    } catch (e: Exception) {
                        Log.e(TAG, "VNodeRenderer: invokeHandler failed id=$id", e)
                        android.widget.Toast.makeText(context, "Error: ${e.message}", android.widget.Toast.LENGTH_SHORT).show()
                    }
                }
            }
        }
    }

    private fun styleFromProps(props: JSONObject?): JSONObject? {
        if (props == null) return null
        val styleObj = props.optJSONObject("style") ?: (props.opt("style") as? JSONObject)
        return styleObj?.optJSONObject("value") ?: styleObj
    }

    private fun styleInt(style: JSONObject?, key: String): Int? {
        if (style == null) return null
        val v = style.opt(key) ?: return null
        return when (v) {
            is Number -> v.toInt()
            is JSONObject -> v.optInt("value", -1).let { if (it == -1) null else it }
            is String -> v.toIntOrNull()
            else -> null
        }
    }

    private fun styleValue(style: JSONObject, key: String): String {
        val v = style.opt(key) ?: return ""
        return when (v) {
            is String -> v
            is JSONObject -> v.optString("value", v.optString("String", ""))
            else -> v.toString()
        }
    }

    private fun styleDouble(style: JSONObject, key: String): Double? {
        val v = style.opt(key) ?: return null
        return when (v) {
            is Number -> v.toDouble()
            is JSONObject -> v.optDouble("value", Double.NaN).let { if (it.isNaN()) null else it }
            is String -> v.toDoubleOrNull()
            else -> null
        }
    }

    private fun propString(props: JSONObject, key: String): String? {
        val v = props.opt(key) ?: return null
        return when (v) {
            is String -> v
            is JSONObject -> v.optString("value").ifEmpty { v.optString("String") }.ifEmpty { null }
            else -> v.toString()
        }
    }

    private fun dp(px: Int): Int = TypedValue.applyDimension(
        TypedValue.COMPLEX_UNIT_DIP, px.toFloat(), context.resources.displayMetrics
    ).toInt()
}
