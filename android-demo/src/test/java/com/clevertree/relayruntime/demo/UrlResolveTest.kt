package com.clevertree.relayruntime.demo

import org.junit.Assert.assertEquals
import org.junit.Test

class UrlResolveTest {

    @Test
    fun resolve_relative() {
        assertEquals(
            "https://example.com/a/hooks/app.jsx",
            UrlResolve.resolve("https://example.com/a", "hooks/app.jsx"),
        )
    }

    @Test
    fun resolve_root_relative() {
        assertEquals(
            "https://example.com/static/hooks/app.jsx",
            UrlResolve.resolve("https://example.com/a/b", "/static/hooks/app.jsx"),
        )
    }

    @Test
    fun resolve_absolute_url_unchanged() {
        assertEquals(
            "https://other.test/x.jsx",
            UrlResolve.resolve("https://example.com/a", "https://other.test/x.jsx"),
        )
    }

    @Test
    fun normalizeModuleKey_strips_leading_slash_and_dot_slash() {
        assertEquals("hooks/x.jsx", UrlResolve.normalizeModuleKey("/hooks/x.jsx"))
        assertEquals("hooks/x.jsx", UrlResolve.normalizeModuleKey("./hooks/x.jsx"))
    }
}
