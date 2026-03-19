# relay-runtime

Universal client hook runtime: parsing, execution, rendering, and debugging for Relay client hooks.

## Goal

Render all client hooks in `relay-template/hooks/client/*` with **relay-runtime** as the **only** execution engine (hooks, tree, reconciliation, **theme styling**). **No** hook transpile-to-JS, **no** legacy JS runtimes for hooks ([POLICY.md](docs/POLICY.md)). **Theme styling is built into relay-runtime** (no separate themed-styler dependency). **Native:** no JS engine on the hook path. **Web:** **relay-runtime** in **WASM** (parsing, execution, styling); browser **JavaScript is glue only** ([WEB_WASM_HOST.md](docs/WEB_WASM_HOST.md)).

## Docs

- [POLICY.md](docs/POLICY.md) — No legacy; no transpiler; theme styling built-in
- [CLEVERSCRIPT.md](docs/CLEVERSCRIPT.md) — **CleverScript** language spec: supported subset, out-of-scope constructs
- [NEXT_STEPS.md](docs/NEXT_STEPS.md) — Ordered work (this repo + Relay clients)
- [PLAN.md](docs/PLAN.md) — Strategy and repo roles
- [WEB_WASM_HOST.md](docs/WEB_WASM_HOST.md) — Web: WASM core, minimal JS host
- [IMPLEMENTATION_PHASES.md](docs/IMPLEMENTATION_PHASES.md) — Phased implementation and test scenarios

## Phases

1. **Parser** ✅ — Lexer + parser for JS-like + JSX
2. **IR** ✅ — Semantic analysis, hook rules, component detection
3. **Hooks Runtime** ✅ — useState (useEffect, useCallback stubbed)
4. **Host Bindings** ✅ — createElement, fetch, navigate via Host trait
5. **Renderer** — VDOM + `vdom::reconcile` ✅; DOM commit via **versioned host API** (web: JS applies patches only — see `WEB_WASM_HOST.md`)
6. **Module System** ✅ — import(), load_module, with_context(params, env, navigate)
7. **Integration** — Fixtures under `tests/fixtures/client/`; `mini_ok.jsx` e2e ✅; full Layout/query-client when parser grows

## Build

```bash
cargo build
cargo test
```

### WASM (web)

```bash
./scripts/test-wasm.sh   # Builds WASM + web wrapper, starts server at http://localhost:8765/examples/wasm-test/
```

The build produces `relay-runtime-web.js` with `bootstrap()` and `vnodeToDom()`. **Integration is minimal**—the page only loads the library and calls `bootstrap()`:

```html
<script type="module">
  import { bootstrap } from './pkg/relay-runtime-web.js';
  bootstrap();
</script>
```

Theme styling, fetch logic, VNode-to-DOM, and app orchestration all live in relay-runtime. See [docs/WEB_WASM_HOST.md](docs/WEB_WASM_HOST.md).

## License

MIT OR Apache-2.0
