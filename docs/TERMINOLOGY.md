# Relay terminology

Shared glossary for Relay repos. The Android ↔ web tester is [ANDROID_WEB_TEST.md](ANDROID_WEB_TEST.md) — keep hook **examples** that the harness actually runs there.

A **Relay app** is a **git repo** (a tip / site / library catalog — not an APK). Behavior is files in that repo, wired by **`.relay.yaml`**. The gateway and every client host execute those files; they do not ship a per-app binary UI.

| Term | Meaning |
|------|---------|
| **Hook** | A CleverScript module in the repo that the runtime runs. Source of truth for that repo’s behavior. |
| **Client hook** | UI and client-side flow. Usual path **`hooks/client/*.jsx`** (default GET: **`hooks/client/get.jsx`**). Default export builds a VNode tree. Runs in **relay-runtime** on **WASM (web)** and **JNI (Android)**. `fetch`, `navigate`, theme, composition. Must not touch `document` / `window`. |
| **Server hook** | Gateway-side request and policy. Usual path **`hooks/server/*.cs`**, listed under **`server.hooks.*`** in `.relay.yaml` (`get`, `query`, `pre-commit`, …). Same language; extra **`@relay/git`**, search, image, SSR modules. Returns HTTP / QUERY / validation — not a native view tree. |
| **`.relay.yaml`** | Maps hook **roles** to paths (`client.hooks.get.path`, `server.hooks.get.path`, …). That is how a repo *is* an app. |
| **Host** | Thin platform glue: web JS applies VNodes to the DOM; Android Kotlin applies them to Views. Glue does not author the app. |
| **`@relay/*` helper** | Rust (in relay-runtime or the gateway) imported from CS. Sorting, paths, dates, catalog, markdown, search, TTS, etc. |

## Why CleverScript (not JavaScript on device)

JavaScript on Android meant a **custom Hermes** (and earlier JSC) stack. That library did not really work **outside Facebook**. Shipping a JS engine + bridge + React-like runtime for hooks was **too heavy** and looked like a **dead end**.

Absolute **parity** (one hook source, web and Android) needed a language that is **very similar to JavaScript** so authors keep JSX/hooks habits, but **strictly not JS**: a small, specified subset parsed and run in **Rust** (`relay-runtime`). No Hermes, no V8, no hook-transpiler-to-JS on the product path.

CleverScript is **not** a fast JS engine. It is **far less performant** than running real JavaScript, on purpose. Menial and hot work belongs in **Rust helpers**. CS itself is **routing**, **enabling features**, **advanced control flow**, and **orchestration**. Heavy **processing** (parse, search, dates, catalog walks, image, markdown) almost always runs in Rust; the hook **calls** it.

## Related

- Spec: [CLEVERSCRIPT.md](CLEVERSCRIPT.md)
- Policy: [POLICY.md](POLICY.md)
- Gateway wiring: [SERVER_HOOKS.md](../../relay-gateway/docs/SERVER_HOOKS.md)
- Helpers: [CS_HOST_HELPERS.md](../../relay-gateway/docs/CS_HOST_HELPERS.md)
