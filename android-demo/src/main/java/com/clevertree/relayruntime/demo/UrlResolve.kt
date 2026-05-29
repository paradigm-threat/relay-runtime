package com.clevertree.relayruntime.demo

import java.net.URI

/**
 * Resolve hook / theme / manifest paths against a remote base URL.
 * Matches web [resolveHookUrl]: absolute URLs, root-relative (`/foo`), and relative (`./x`, `../x`).
 */
object UrlResolve {
    fun resolve(baseUrl: String, path: String): String {
        val p = path.trim()
        if (p.isEmpty()) return baseUrl.trimEnd('/')
        if (p.startsWith("http://", ignoreCase = true) || p.startsWith("https://", ignoreCase = true)) {
            return p
        }
        val base = baseUrl.trimEnd('/')
        val dir = URI("$base/")
        return dir.resolve(p).normalize().toString()
    }

    fun normalizeModuleKey(path: String): String {
        val s = path.trim()
        return when {
            s.startsWith("/") -> s.drop(1)
            s.startsWith("./") -> s.drop(2)
            else -> s
        }
    }
}
