package com.clevertree.relayruntime

/**
 * JNI bridge to relay-runtime. Loads librelay_runtime.so and exposes native methods.
 */
object RelayRuntime {

    init {
        System.loadLibrary("relay_runtime")
    }

    /**
     * Run hook with module map, theme, and hooks manifest.
     * @return VNode JSON string, or throws on error
     */
    external fun nativeRunHookWithHost(
        source: String,
        moduleMapJson: String,
        themeYaml: String,
        hooksManifestJson: String,
        file: String,
        themeName: String,
        prefersDark: Boolean,
        displayDensity: Float,
        scaledDensity: Float,
    ): String?

    /**
     * Invoke handler by ID (e.g. from onClick).
     * @return true if successful
     */
    external fun nativeInvokeHandler(handlerId: Int): Boolean

    /**
     * Returns true if setState ran since last run; clears the flag.
     */
    external fun nativeConsumeRenderRequested(): Boolean

    /**
     * Set current theme without triggering render. Call on init to restore saved theme.
     */
    external fun nativeSetTheme(themeName: String)

    /**
     * Get current theme name (from setTheme). Pass to runHookWithHost.
     */
    external fun nativeGetCurrentTheme(): String

    /**
     * Get CSS injected by the hook (may be empty on Android).
     */
    external fun nativeGetInjectedCss(): String

    /**
     * Get theme list JSON. Call after runHookWithHost.
     */
    external fun nativeGetThemes(): String

    /**
     * Set current theme and return new CSS.
     */
    external fun nativeSetThemeAndRender(themeName: String): String

    fun runHookWithHost(
        source: String,
        moduleMapJson: String = "{}",
        themeYaml: String = "",
        hooksManifestJson: String = "{}",
        file: String = "",
        themeName: String = "",
        prefersDark: Boolean = false,
        displayDensity: Float = 2f,
        scaledDensity: Float = 2f,
    ): String {
        return nativeRunHookWithHost(
            source,
            moduleMapJson,
            themeYaml,
            hooksManifestJson,
            file,
            themeName,
            prefersDark,
            displayDensity,
            scaledDensity,
        )
            ?: throw RuntimeException("runHookWithHost returned null")
    }

    fun invokeHandler(handlerId: Int) {
        if (!nativeInvokeHandler(handlerId)) {
            throw RuntimeException("invokeHandler failed for id=$handlerId")
        }
    }

    fun consumeRenderRequested(): Boolean = nativeConsumeRenderRequested()
    fun setTheme(themeName: String) = nativeSetTheme(themeName)
    fun getCurrentTheme(): String = nativeGetCurrentTheme()
    fun getInjectedCss(): String = nativeGetInjectedCss()
    fun getThemes(): String = nativeGetThemes()
    fun setThemeAndRender(themeName: String): String = nativeSetThemeAndRender(themeName)
}
