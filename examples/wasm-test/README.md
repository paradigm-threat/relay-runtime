# relay-runtime WASM test

Verifies DOM output from relay-runtime (WASM) in the browser. Uses **themed-styler** for styles (built-in light/dark themes, Tailwind utilities).

Hooks are written in **CleverScript** — see [docs/CLEVERSCRIPT.md](../../docs/CLEVERSCRIPT.md) for the supported language subset.

## Quick Start

```bash
# From relay-runtime root
./scripts/test-wasm.sh
```

Builds relay-runtime WASM + web wrapper, then starts the server. Open: http://localhost:8765/examples/wasm-test/

## Integration: ultra-slim index.html

The page uses **only** the minimal glue to load relay-runtime. All logic (fetch, run, VNode-to-DOM) lives in the library:

```html
<div id="root">Loading…</div>
<style id="themed-styler-output"></style>
<script type="module">
  import { bootstrap } from './pkg/relay-runtime-web.js';
  bootstrap();
</script>
```

See [docs/WEB_WASM_HOST.md](../../docs/WEB_WASM_HOST.md) for the integration requirement.

## Verify

- All hook tests render in scrollable sections (Simple JSX, Style Keywords, Nested Structure, String Keywords, useState Demo).
- Styles come from theme.yaml via themed-styler.
