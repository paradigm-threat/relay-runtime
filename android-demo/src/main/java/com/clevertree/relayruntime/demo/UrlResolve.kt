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

    fun looksLikeHookSource(text: String): Boolean {
        val t = text.trimStart()
        if (t.isEmpty()) return false
        val head = t.take(64).lowercase()
        return !head.startsWith("<!doctype") && !head.startsWith("<html")
    }

    fun localImportSpecs(source: String): List<String> {
        val out = ArrayList<String>()
        var rest = source
        while (true) {
            val idx = rest.indexOf(" from ")
            if (idx < 0) break
            var after = rest.substring(idx + 6).trimStart()
            val q = when {
                after.startsWith("'") -> '\''
                after.startsWith("\"") -> '"'
                else -> {
                    rest = rest.substring(idx + 6)
                    continue
                }
            }
            val body = after.substring(1)
            val end = body.indexOf(q)
            if (end < 0) break
            val spec = body.substring(0, end)
            if (
                spec.startsWith("hooks/") ||
                spec.startsWith("./") ||
                spec.startsWith("../") ||
                spec.endsWith(".jsx") ||
                spec.endsWith(".js") ||
                spec.endsWith(".yaml") ||
                spec.endsWith(".yml")
            ) {
                out.add(spec)
            }
            rest = body.substring(end + 1)
        }
        return out
    }

    fun resolveImportRel(importer: String, spec: String): String {
        if (spec.startsWith("hooks/") || !spec.startsWith(".")) {
            return spec.trimStart('/')
        }
        val imp = importer.trimStart('/')
        val slash = imp.lastIndexOf('/')
        val base = if (slash < 0) "" else imp.substring(0, slash)
        val joined = if (base.isEmpty()) spec else "$base/$spec"
        val parts = ArrayList<String>()
        for (c in joined.split('/')) {
            when (c) {
                "", "." -> {}
                ".." -> {
                    if (parts.isNotEmpty()) parts.removeAt(parts.size - 1)
                }
                else -> parts.add(c)
            }
        }
        return parts.joinToString("/")
    }
}
