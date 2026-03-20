# Next steps (relay-runtime & Relay clients)

Ordered work aligned with [POLICY.md](POLICY.md): **no legacy**, **no hook transpile to JS**, **relay-runtime + themed-styler** only for hook execution.

---

## relay-runtime (this repo)

1. **Parser/runtime parity** — Language features needed for `relay-template/hooks/client/*` (see [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md)): optional chaining, `!`, default params, template literals, async, dynamic `import`, etc.
2. **DOM / native host commit** — Turn `VNode` + `vdom::reconcile` into a **versioned** host API; implement web glue spec in [WEB_WASM_HOST.md](WEB_WASM_HOST.md).
3. **themed-styler integration** — Crate dependency + WASM/native: resolve class/theme → concrete style payload for host.
4. **WASM build** ✅ — `wasm-pack build --target web`; `run_hook(source)` export; test app in `examples/wasm-test/` (verify in browser).
5. **Hooks** — Complete `useEffect`, `useCallback`, and any semantics required by fixtures.
6. **Re-render on state change** — See [PLAN.md](PLAN.md) §5. Persist runtime state, wire onClick→invoke_handler, host re-run loop.
6. **Integration tests** — Enable ignored Layout/query-client tests as parser grows; headless mock-host tests for full trees.
7. **Docs** — Keep POLICY / PLAN / phases in sync when behavior lands.

---

## relay-client-web

1. **Thin shell** — Load WASM, implement host imports (DOM, fetch, rAF), forward input events to runtime.
2. **Remove** HookRenderer / transpiler / Act-or-React **hook execution** paths ✅ (torn down; minimal placeholder shell).
3. **Bundler** — Vite/esbuild for the **shell** TypeScript/JS only; hook sources are **bytes to WASM**, not bundled as executable JS.

---

## relay-client-android

1. **JNI (or shared API)** to **relay-runtime** native + **themed-styler**.
2. **Remove** JSC + act + bridge.js **hook** pipeline ✅ (torn down; minimal placeholder shell).

---

## hook-transpiler (repo)

- **Not** part of the Relay hook execution architecture. Treat as **deprecated for Relay**; do not add Relay-specific features there. Optionally archive or repurpose for non-Relay tooling only.

---

## themed-styler (repo)

- **Keep** as standalone library; expose APIs relay-runtime needs; document WASM + native usage for Relay hosts.

---

## relay-server / relay-template

- **Unchanged** contract for *serving* hook source; ensure examples stay within the **documented** relay-runtime language subset as it grows.
