# Relay hook stack — policy (no legacy)

This document states **non‑negotiables** for the Relay client-hook stack. **relay-runtime** is greenfield: it does not ship legacy code. Other Relay repos (**relay-client-web**, **relay-client-android**, etc.) should **remove** old paths when they adopt this stack—not keep feature flags or fallbacks.

---

## Single execution pipeline

| ✅ Required | ❌ Not supported (do not build or preserve) |
|-------------|---------------------------------------------|
| **relay-runtime** parses and executes remote hook **source** (JS-like + JSX subset) in **Rust** (native or **WASM**) | **hook-transpiler** (or any step that emits JS for hook execution) on the Relay hook path |
| **Theme styling built into relay-runtime** (no separate themed-styler package; single WASM artifact) | Running hooks inside **JavaScriptCore**, **V8**, or **Act/React in JS** for product UI |
| Host **glue only**: DOM FFI, `fetch`, timers, asset load (web JS; Kotlin JNI on native) | **jscbridge** + **act** + **bridge.js** `__hook_jsx_runtime` style pipelines for **new** work |
| One parser + one runtime semantics in **relay-runtime** | Dual “transpile then run in JS” and “run in runtime” modes |

**Hooks are not transpiled to JavaScript anywhere for execution.** If the parser cannot accept a construct, **extend relay-runtime** (or narrow authoring)—do not add a transpiler shim.

---

## Web

- **WASM** = relay-runtime (parsing, execution, **theme styling** in one artifact). **No** hook-transpiler WASM artifact. **No** separate themed-styler build.
- Browser **JavaScript** = shell + platform bindings only (see [WEB_WASM_HOST.md](WEB_WASM_HOST.md)).

## Native

- **relay-runtime** (theme styling compiled in). No embedded JS engine for hook execution.

## Theme styling

- **Built into relay-runtime.** Styling logic is compiled in; consumers use a single package. No separate themed-styler npm/WASM artifact.

---

## Out of scope for Relay (documentation only)

The following are **historical** or **other products**—not part of the Relay hook roadmap:

- **Option B** in [archive/PLAN.md](archive/PLAN.md) (compile hook language to JS for execution) — **not** a Relay target.
- **Migration feature flags** (“WASM renderer until parity with old path”) — **no** old path; parity is **required** before cutover, not a long-lived dual stack.

---

## Related

- [CLEVERSCRIPT.md](CLEVERSCRIPT.md) — CleverScript language spec (supported subset, out-of-scope)
- [WEB_WASM_HOST.md](WEB_WASM_HOST.md) — web embedding
- [NEXT_STEPS.md](NEXT_STEPS.md) — ordered work
- [archive/PLAN.md](archive/PLAN.md) — historical strategy
