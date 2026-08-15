# relay-runtime

Universal client hook runtime: parsing, execution, rendering, and debugging for Relay client hooks.

## Goal

Render all client hooks in `relay-template/hooks/client/*` with **relay-runtime** as the **only** execution engine (hooks, tree, reconciliation, **theme styling**). **No** hook transpile-to-JS, **no** legacy JS runtimes for hooks ([POLICY.md](docs/POLICY.md)). **Theme styling is built into relay-runtime** (no separate themed-styler dependency). **Native:** no JS engine on the hook path. **Web:** **relay-runtime** in **WASM** (parsing, execution, styling); browser **JavaScript is glue only** ([WEB_WASM_HOST.md](docs/WEB_WASM_HOST.md)).

## Docs

- [ANDROID_WEB_TEST.md](docs/ANDROID_WEB_TEST.md) — in-tree demo APK vs wasm-test; same hooks; web is the visual standard; skip native desktop. **Update this file when Android testing progresses.**
- [CROSS_PLATFORM_OS_SUPPORT_PLAN.md](docs/CROSS_PLATFORM_OS_SUPPORT_PLAN.md) — Style core-only policy; platform glue applies pre-resolved styles only
- [POLICY.md](docs/POLICY.md) — No legacy; no transpiler; theme styling built-in
- [CLEVERSCRIPT.md](docs/CLEVERSCRIPT.md) — **CleverScript** language spec: supported subset, out-of-scope constructs (prefer JSX over template literals for templating)
- [NEXT_STEPS.md](docs/NEXT_STEPS.md) — Ordered work (this repo + Relay clients)
- [PLAN.md](docs/PLAN.md) — Strategy and repo roles
- [WEB_WASM_HOST.md](docs/WEB_WASM_HOST.md) — Web: WASM core, minimal JS host
- [PARALLEL_PROJECT_ONE_WORLD.md](docs/PARALLEL_PROJECT_ONE_WORLD.md) — Brother's WebGL one-world / ABACUS stack and Relay integration milestone
- [IMPLEMENTATION_PHASES.md](docs/IMPLEMENTATION_PHASES.md) — Phased implementation and test scenarios

## Phases

1. **Parser** ✅ — Lexer + parser for JS-like + JSX
2. **IR** ✅ — Semantic analysis, hook rules, component detection
3. **Hooks Runtime** ✅ — useState (useEffect, useCallback stubbed)
4. **Host Bindings** ✅ — createElement, fetch, navigate via Host trait
5. **Renderer** — VDOM + `vdom::reconcile` ✅; DOM commit via **versioned host API** (web: JS applies patches only — see `WEB_WASM_HOST.md`)
6. **Module System** ✅ — import(), load_module, with_context(params, env, navigate)
7. **Integration** — Fixtures under `tests/fixtures/client/`; `mini_ok.jsx` e2e ✅; full Layout/query-client when parser grows

## Dev layout

**relay-runtime** is a **sibling checkout** of the Relay monorepo (`relay/`), not nested under `relay-gateway/`:

```
dev/
  relay/                 # relay-gateway, relay-client-web, …
  relay-runtime/         # this crate (CleverScript + WASM)
  pt/paradigm-threat-files/
  md2ast/
```

**relay-gateway** depends on `../../relay-runtime` (path in `relay-gateway/Cargo.toml`). **relay-client-web** builds WASM via `npm run build:wasm` (runs `relay-runtime/scripts/build-wasm-web.sh`).

## Build

```bash
cargo test          # run before build:wasm when changing Rust
cargo build
```

Paradigm Threat hook fixtures under `tests/fixtures/paradigm-threat-files/hooks` mirror **paradigm-threat-files** — refresh after editing that repo:

```bash
./scripts/sync-pt-fixtures.sh
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

### Android

```bash
./scripts/build-android.sh        # Rebuild librelay_runtime.so after native/JNI changes (required when JNI signatures change)
./scripts/build-install-demo.sh   # Build APK, install on device/emulator, launch
```

Run after each change to rebuild and redeploy the demo APK.

Hooks are loaded from `android-demo/src/main/assets/hooks/`. **Remote hooks:** set `RELAY_REMOTE_HOST` on the demo activity intent (base URL, e.g. `http://10.0.2.2:8765/examples/wasm-test/pkg`) and optionally `RELAY_HOOK_PATH` (default `hooks/app.jsx`). The demo fetches `theme.yaml`, `hooks.json`, and each hook path using the same URL resolution rules as the web wrapper (`resolveHookUrl` / `UrlResolve`).

To keep hooks in sync with relay-client-web and hook-transpiler tests:

```bash
./scripts/sync-hooks.sh   # Copies relay-client-web/public/hooks to relay-runtime + hook-transpiler
```

Source of truth: `relay-client-web/public/hooks/`. The demo entry is `hooks/demo.jsx` (simple counter).

## License

MIT OR Apache-2.0
