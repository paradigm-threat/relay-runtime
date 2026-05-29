# relay-runtime Copilot Instructions

## Project Overview

Universal client hook runtime for Relay hooks: parsing, execution, rendering (VNode), and debugging.
Targets: web (WASM), Android (JNI), desktop (future). Theme styling is built into relay-runtime via themed-styler.

## Architecture

- **Host trait** — Abstracts fetch, navigate, create_element, load_module
- **VNode** — Platform-agnostic tree; hosts convert to DOM/native views
- **themed-styler** — Dependency for CSS/theme resolution (WASM + Android features)
- **run_state** — Persistent state, consume_render_requested(), invoke_handler(id)

## Style Handling: Core-Only (CRITICAL)

**All style handling MUST occur in relay-runtime core.** Platform glue (Android .kt, web host, etc.) must NOT interpret className or Tailwind classes.

**class + tag = stylemap:** For each element, runtime resolves `(tag, className)` via themed-styler:
- **Web:** className kept; cascading CSS emitted
- **Non-web:** flat style map per element (no cascade); `props.style` contains resolved values

Platform is implicit from build target. Core binary does all hard work.

| Layer | Responsibility |
|-------|----------------|
| **relay-runtime (this repo)** | `style_resolve::resolve_vnode_styles` → themed-styler → platform-ready output |
| **Platform glue** | Apply `props.style` directly. No className parsing. |

See `docs/CROSS_PLATFORM_OS_SUPPORT_PLAN.md` §2.

## Build & Test

```bash
cargo build
cargo test
./scripts/test-wasm.sh          # Web WASM + dev server
./scripts/build-install-demo.sh # Android APK + install
```

## Key Paths

- `src/` — Rust core (parser, runtime, vdom, value)
- `src/vdom/` — value_to_vnode, PropValue, VNode
- `src/android_jni.rs` — Android JNI exports
- `android-demo/` — APK demo; VNodeRenderer.kt applies style props only (no className logic)

## JSX Tag Standards

- Use `<span>` for text (mapped to TextView on Android)
- **NEVER** use `<text>` — not standard HTML
- Use `<div>` or `<view>` for containers (LinearLayout on Android)
