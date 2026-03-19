# Web host: WASM core, minimal JavaScript

**Policy:** See **[POLICY.md](POLICY.md)** — **no legacy** paths, **no** hook-transpiler, **no** transpile-to-JS for execution. **relay-runtime** (WASM) parses and runs remote hook **source**; **themed-styler** is a **separate** library (linked in WASM or native).

This document defines the **target architecture** for **relay-client-web**: **all core rendering and styling semantics** run inside **WebAssembly** from our Rust stack. **Browser JavaScript is not a semantic dependency**—it does not implement hooks, reconciliation, or style resolution. It only **loads WASM**, **implements the host surface WASM cannot provide**, and **bridges** the DOM and platform APIs.

---

## Integration requirement: ultra-slim page glue

**All integration logic lives in relay-runtime.** The published NPM package includes `relay-runtime-web.js`, which provides:

- `bootstrap(options?)` — loads WASM, fetches theme/hooks/app, runs app, injects CSS, renders VNode to DOM
- `vnodeToDom(v)` — converts VNode JSON to DOM nodes

**The page should use only the minimal glue required to load relay-runtime.** No custom fetch logic, VNode-to-DOM, or main async function in the page.

**Minimal index.html:**

```html
<div id="root">Loading…</div>
<style id="themed-styler-output"></style>
<script type="module">
  import { bootstrap } from 'relay-runtime';
  bootstrap();
</script>
```

When using the built package (e.g. from `./pkg/relay-runtime-web.js`):

```html
<script type="module">
  import { bootstrap } from './pkg/relay-runtime-web.js';
  bootstrap();
</script>
```

Optional `bootstrap({ rootId, styleId, themePath, hooksPath, appPath, baseUrl })` for custom paths.

---

## Principles

| In WASM (our code) | Outside WASM (host JS / browser) |
|--------------------|----------------------------------|
| **relay-runtime** — parse hook source, execute hooks, **VNode** tree, **reconcile** | Instantiate WASM, memory, string/byte marshalling |
| **themed-styler** — theme YAML, class/style resolution → concrete style payloads | Load theme assets if needed; optional cache |
| Decide *what* the UI tree is and *what* styles mean | **`document.createElement` / `setAttribute` / `textContent`**, or minimal DOM API via **imports** |
| Event handler **identity** and dispatch logic owned by runtime | Subscribe DOM events, forward to WASM **entry points** (e.g. `on_click(node_id)`) |

**Not in this architecture:** **hook-transpiler**, Act/React, or any **second** implementation of hooks or reconciliation in JS.

---

## Data flow (high level)

1. Host JS: fetch hook **source** (UTF-8 bytes) → WASM **relay-runtime** (parse + run) → **VNode** + style resolution via **themed-styler** (crate inside or beside same WASM build).
2. WASM: **diff** previous vs next tree (`vdom::reconcile`) → **patch list** or batched host calls.
3. Host JS: **apply patches** to the real DOM (listeners call back into WASM).

All **correctness** of hooks order, state updates, and tree shape stays in Rust/WASM.

---

## Pros (why this split)

- Single source of truth with **Android/native** (same relay-runtime + themed-styler).
- No duplicate reconciler in JS; no transpiler to keep in sync.
- Auditable **WASM + small glue** for the hook security surface.

## Cons / costs (plan for them)

- **FFI churn**: batch DOM updates; avoid per-prop round-trips without profiling.
- **Binary size**: one WASM module (runtime + styler) or split modules + shared allocator—plan explicitly.
- **Debugging**: logging from WASM + clear **versioned host API** docs.

---

## Implementation checklist (relay-runtime + relay-client-web)

1. **Freeze a web `Host` contract**  
   - Align `relay_runtime::host::Host` (or `wasm_host`) with minimal **DOM operations**: create/update/remove nodes, text, attributes/styles from resolved maps, listeners by stable id.  
   - **Version** the import module for `wasm-bindgen` (or chosen toolchain).

2. **DOM commit from `VNode` + `vdom::reconcile`**  
   - `patches` → host calls from WASM, or typed patch buffer consumed by glue (logic stays in WASM).

3. **relay-client-web shell**  
   - Init, host imports, `fetch`, rAF, devtools bridge only. **No** hook execution in JS.

4. **WASM artifact**  
   - **relay-runtime** + **themed-styler** (dependency). No separate transpiler artifact.

5. **themed-styler**  
   - Theme + class names from runtime → host-consumable style map / CSS variables in glue.

6. **Tests**  
   - Headless: WASM + mock host. Browser: e2e on **shell + WASM** only.

7. **relay-client-web teardown**  
   - Remove old HookRenderer / transpiler / JS hook runtime per **[POLICY.md](POLICY.md)** and **[NEXT_STEPS.md](NEXT_STEPS.md)**.

---

## Related docs

- [POLICY.md](POLICY.md) — non-goals and “no legacy”
- [NEXT_STEPS.md](NEXT_STEPS.md) — cross-repo work order
- [PLAN.md](PLAN.md) — strategy
- [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md) — Phase 5 web track
