# Relay-Runtime Cross-Platform OS Support Plan

**Goal:** Build and test relay-runtime on all major OS targets (web, Android, Windows, macOS) with a focus on **rendering efficiency**, **accuracy**, and **minimal rerender overhead**. Rely on native components for performance-critical domains (audio, video, gaming).

**Rendering strategy:** Theme styling is **compiled into relay-runtime** via the **themed-styler** path crate ([POLICY.md](POLICY.md)) — no separate npm/WASM consumer artifact. No webview, no Tauri.

---

## 1. Current State Summary

| Platform | Status | Notes |
|----------|--------|-------|
| **Web (WASM)** | ✅ Working | `relay-client-web` + vendored `relay-runtime-web.js`; fan-edits e2e |
| **Desktop (Linux)** | ✅ Working | `relay-runtime-desktop` + `relay-client-desktop`; fan-edits `--verify` |
| **Android (demo)** | ✅ Working | `relay-runtime/android-demo` APK |
| **Android (product)** | ✅ Wired | `relay-client-android` + JNI; remote fan-edits gateway |
| **Native CLI** | ✅ Working | `relay-cli`, integration tests |
| **Windows / macOS desktop** | ❌ Not implemented | Linux desktop crate exists |

### Architecture (unchanged)

- **Host trait** abstracts `fetch`, `navigate`, `create_element`, `load_module`
- **VNode** is platform-agnostic; hosts convert to DOM/native views
- **Reconcile** (`vdom/reconcile.rs`): `diff`, `patches`, `vnodes_equal` for incremental updates
- **themed-styler**: WASM + Android (JNI) support; dependency of relay-runtime
- **run_state**: persistent state, `consume_render_requested()`, `invoke_handler(id)`

**UI authorship:** CleverScript hooks describe Relay UI without targeting a platform. Hosts (web
glue, Android, desktop) apply VNodes and natives. Hooks must not use `document`/`window` (or
equivalent) directly; page scripts must not read WASM/runtime private state — only the published
host API ([`RELAY_EMBED_CLIENT_GET_INTERCEPT.md`](../../relay-gateway/docs/RELAY_EMBED_CLIENT_GET_INTERCEPT.md)
§ Host boundary).

**Parked web / embed:** content sites (e.g. clevertree.net) hydrate via **WASM only** — no npm React on
`/assets/embed.js`. If the React npm runtime is ever required for a host feature, treat it as a
**native component** (CleverScript configures; host owns the React tree), same pattern as
Android/desktop widgets — **deferred**; see gateway embed doc § Dependency policy.

---

## 2. Styling and Rendering Strategy

### 2.0 Style Resolution: Core-Only (CRITICAL)

**All style handling must occur in relay-runtime core.** Platform glue (Android `.kt`, web host, desktop) must NOT interpret className or Tailwind classes.

| Layer | Responsibility |
|-------|----------------|
| **relay-runtime core** | Parse className + inline style → themed-styler → output resolved `style` prop with platform-ready values (standard CSS property names: padding, margin, backgroundColor, fontSize, etc.). Must support translation to all standard OS style property formats. |
| **Platform glue (.kt, host)** | Apply pre-resolved `style` prop directly to View/DOM. No className parsing. No Tailwind interpretation. No translation unless absolutely necessary (e.g. deserializing JSON → View attribute). |

**VNode output contract:** Each element's `props.style` contains resolved values. Platform receives `style: { padding: 16, marginBottom: 8, backgroundColor: "#fff", fontSize: 14 }` and maps directly to `view.setPadding()`, `layoutParams.bottomMargin`, `view.setBackgroundColor()`, etc. Numbers are in dp (Android) or px (web) as appropriate for target.

### 2.1 themed-styler as Single Source

**theme.yaml** uses CSS-standard keys/values. **relay-runtime** (not platform glue) invokes themed-styler to resolve className + theme → CSS values. Each platform receives already-resolved style values:

| Platform | Runtime Output | Platform applies |
|----------|----------------|------------------|
| **Web** | style object (px, hex) | `el.style.*` inline |
| **Android** | style object (dp, hex colors) | `view.setPadding`, `setBackgroundColor`, `layoutParams` |
| **Windows / macOS** | style object | Skia / native attributes |

**Principle:** Runtime produces platform-ready style values. Platform glue does direct mapping only.

**Implemented:** relay-runtime resolves `className` + tag → style in core via themed-styler. Platform is implicit from build target (web vs android).

### 2.2 Flat Style Resolution (All Platforms)

**All platforms use flat inline style.** No cascading, no CSS injection. Web and native behave the same: className + tag → resolved style per element. themed-styler's `web_styles_for` (web) and `android_styles_for` (Android/iOS/desktop) return a flat style map; runtime emits it as `props.style`. Platform glue applies directly.

**Merge order (lowest to highest priority):** tag defaults → theme selectors (e.g. `.bg-primary`) → Tailwind dynamic utilities (e.g. `p-4`, `mb-2`) → inline `style` prop. Later wins. No cascade tree.

### 2.3 Feature Parity by Platform

- **Web / Android / Desktop:** Same feature set; limited, no cascading. Runtime translates to platform format. Priority:
  - **Flex layout** — flexDirection, justifyContent, alignItems, flex, gap
  - **Positioning** — position (relative, absolute), top/left/right/bottom
  - **Spacing** — margin, padding, width, height, min/max
  - **Typography** — font-size, font-weight, color
  - **Visual** — background-color, border-radius, border-width, box-shadow (where native supports it)

- **Tailwind-based syntax:** themed-styler parses and maps these. **relay-runtime** invokes themed-styler and outputs resolved `style`; platforms never see raw className.

### 2.4 Platform Style Prop Contract

The runtime outputs `props.style` as an object. Standard keys (camelCase, CSS-like):

- **Spacing:** `padding`, `paddingTop`, `paddingBottom`, `paddingLeft`, `paddingRight`, `margin`, `marginTop`, `marginBottom`, `marginLeft`, `marginRight` (numbers in dp/px per platform)
- **Typography:** `fontSize`, `fontWeight`, `color`
- **Visual:** `backgroundColor`, `borderRadius`, `borderWidth`
- **Layout:** `width`, `height`, `minWidth`, `minHeight`, `flex`, `flexDirection`, `justifyContent`, `alignItems`

Platform glue maps these 1:1 to View/DOM APIs. No interpretation.

### 2.5 Desktop Renderer: Skia

- **Graphics backend:** Skia (via `skia-safe` or equivalent).
- **Layout engine:** Interpret flex/positioning from themed-styler output; compute x/y/width/height. Consider `taffy` or custom flex implementation.
- **Text:** Limited but common font set to start (e.g. system fallbacks: Inter, Roboto, SF Pro, Segoe UI). Expand over time.

### 2.6 Input and Handler Wiring

- Hit-test coordinates → VNode node → resolve handler ID.
- On click/focus: call `invoke_handler(id)` → runtime re-runs → new VNode → host applies patches.
- Same pattern as web and Android.

---

## 3. Efficiency Principles

### 3.1 Optimal Performance Without State Changes

The runtime is designed to perform optimally when **no state changes or rerenders** occur:

- **Parse once** → **Analyze once** → **Run once** → **VNode tree** → **Host applies**
- With no `setState` calls, a single run produces the final tree; host renders once.
- This path should be fast on every platform: minimal allocations, no diffing.

### 3.2 Minimizing Rerender Cost

When state *does* change:

- **Patches, not full replace:** Use `vdom::reconcile::patches(old, new)` and apply only changed subtrees.
- **Handler IDs:** Stable handler registration; no re-attaching handlers unnecessarily.
- **Debounce / batch:** Host can debounce rapid `consume_render_requested()` → `run_hook_with_host` cycles.
- **Frequent rerun warning:** Already present (`RERUN_WARN_THRESHOLD_MS`); helps detect abusive refresh patterns.

### 3.3 Native Components for Performance-Critical Domains

**Principle:** Audio, video, gaming, and other high-throughput APIs should **bypass the VNode render loop** and use **native host components**.

| Domain | Approach |
|--------|----------|
| **`<audio>`, `<video>`** | Map to native `MediaPlayer`, `ExoPlayer` (Android), `AVPlayer` (iOS), HTML5 `<video>` (web). Host creates element once; controls (play/pause) via handler IDs, no full re-render. |
| **Canvas / games** | `<canvas>`, `<GameView>`, etc.: Host creates native view, passes ref/canvas API to hook; hook drives game loop via `requestAnimationFrame` or native callbacks. No VNode diff for frame updates. |
| **Heavy lists** | Consider `RecyclerView` (Android), `UITableView` (iOS), virtual scrolling—host can optimize list rendering beyond simple VNode recursion. |
| **Animations** | CSS transitions (web), native animators (Android/iOS): style changes only; no full tree reconciliation. |

**Host extension (future):** Add `Host::create_native_component(tag, props, children)` for tags like `video`, `audio`, `canvas`, `GameView` that the host renders natively and that do not participate in VNode diffing for their inner state.

---

## 4. Target Implementation Plan

### Phase A: Android Native Library + Minimal APK

#### A.1 relay-runtime Android build

1. **Add `android` feature and JNI crate:**
   - `Cargo.toml`: `[target.'cfg(target_os = "android")'.dependencies]` with `jni`, `themed-styler = { path = "../themed-styler", features = ["android"] }`
   - Create `src/android_jni.rs` (conditional `#[cfg(target_os = "android")]`)

2. **JNI API surface (mirror WASM):**
   - `run_hook(source, module_map_json, theme_yaml, hooks_manifest_json, file)` → JSON string
   - `invoke_handler(handler_id)` → void
   - `consume_render_requested()` → boolean
   - `get_injected_css()` → string
   - `get_themes()`, `set_theme_and_render(name)` → string
   - Optional: `run_hook_with_host` equivalent if Kotlin provides Host impl

3. **Build artifacts:**
   - Cross-compile: `cargo build --target aarch64-linux-android` (and armeabi-v7a, x86, x86_64)
   - Output: `librelay_runtime.so` per ABI
   - Place in `android/jniLibs/{abi}/librelay_runtime.so`

4. **Gradle module:**
   - Create `android/` directory with `build.gradle` (Android library), similar to hook-transpiler
   - `sourceSets { main { jniLibs.srcDirs = ['jniLibs'] } }`

#### A.2 Minimal APK project

1. **New repo or subproject:** `relay-runtime-runtime-demo` (or `relay-runtime/android-demo`)
   - Minimal app: single Activity, no server integration initially
   - Single entry: `hooks/app.jsx`; host loads only this file (discovery for imports via asset listing)

2. **Kotlin Host implementation:**
   - Implement `Host` equivalent: `create_element` → Android View (ViewGroup, TextView, etc.)
   - VNode → View hierarchy: recursive `vnodeToView(vnode)` in Kotlin
   - Handler wiring: `view.setOnClickListener { invoke_handler(id) }`
   - Theme: use themed-styler Android output (`android_base_styles`, etc.)

3. **Demo page requirements:**
   - **Render all hooks:** Fetch hooks from manifest; render each in a section or tab
   - **Basic runtime functions demo:** useState counter, onClick handlers, theme toggle, list rendering, conditional JSX
   - **No network:** All hooks loaded from assets for minimal APK

4. **Build script:**
   - `scripts/build-android.sh`: build Rust libs for all ABIs, run Gradle assembleDebug
   - Output: `android-demo/build/outputs/apk/debug/runtime-demo.apk`

---

### Phase B: Windows Native Binary (Custom Skia Renderer)

**No Tauri.** Desktop uses custom rendering driven by themed-styler.

#### B.1 Static/dynamic library

1. **Cargo.toml:**
   - `[lib] crate-type = ["cdylib", "staticlib"]` for `target_os = "windows"`
   - Add `skia-safe` (or equivalent) for desktop target

2. **C FFI layer** (`src/ffi.rs` or `src/native_api.rs`):
   - `#[cfg(not(target_arch = "wasm32"))]` only
   - Expose C-compatible functions: `relay_run_hook`, `relay_invoke_handler`, `relay_consume_render_requested`
   - Use `#[no_mangle] pub extern "C"` for ABI stability

3. **Desktop render crate** (`relay-runtime-desktop` or in-tree):
   - Win32 window (winit) + Skia surface
   - VNode + themed-styler output → layout (flex/positioning) → Skia draw calls
   - Hit-test → handler ID → `invoke_handler`

#### B.2 Skia-based renderer

1. **Layout:** themed-styler outputs computed styles (flex, margin, padding, etc.); layout engine produces x/y/width/height
2. **Paint:** Skia draws rectangles, text, borders, rounded corners; map CSS color, border-radius, etc.
3. **Text:** Bundled/common fonts (Inter, Roboto, system fallbacks). Start with limited set; expand over time.
4. **Input:** winit for events; map click coordinates to VNode path; call `invoke_handler(id)`; re-run and re-render.

---

### Phase C: macOS Native Binary

1. **Same C FFI and renderer as Windows** — unified `src/ffi.rs` and `relay-runtime-desktop` logic
2. **Skia** works on macOS (aarch64 + x86_64)
3. **Build targets:**
   - `aarch64-apple-darwin` (Apple Silicon)
   - `x86_64-apple-darwin` (Intel)
4. **Window:** winit (cross-platform) for both Windows and macOS

---

### Single-Entry Architecture

**Host loads one hook file only.** Default path: `hooks/app.jsx`. No hooks.json or manifest. That entry JSX loads the full UI and other hooks via `import()`.

- **Web:** `bootstrap()` fetches `hooks/app.jsx`; module map built on demand from same directory when imports resolve.
- **Android:** HookRenderer loads `hooks/app.jsx`; discovers sibling hooks by listing `assets/hooks/` so `import('./home.jsx')` etc. resolve.
- **Flow:** Host knows only the entry path. The entry hook orchestrates everything via imports.

---

### Phase D: All-Hooks Demo Page (Web + Android)

#### D.1 Web (WASM)

1. Extend `examples/wasm-test/`:
   - Single entry: `hooks/app.jsx` (or `hooks/client/get.jsx`)
   - App entry imports and renders Home, Settings, peerContent, etc.
   - Demo: useState, onClick, theme toggle, list, conditional, async (if supported)

2. **Basic runtime functions checklist:**
   - [ ] useState (counter)
   - [ ] onClick handler
   - [ ] Theme switch (data-theme, setCurrentTheme)
   - [ ] List with map + key
   - [ ] Conditional render (`{x && <Y />}`)
   - [ ] fetch (mock or real)
   - [ ] navigate (mock)

#### D.2 Android (Minimal APK)

1. Single entry: `hooks/app.jsx` in `assets/hooks/`; host loads only this path.
2. HookRenderer discovers all `.jsx` in `assets/hooks/` for `import()` resolution.
3. App (app.jsx) renders tabs, Home, Settings, peer content via imports.

---

### Phase E: Error Boundary (Built-In)

**Decision:** Error boundary is built into relay-runtime. App hooks no longer import `ErrorBoundary` from a separate file.

#### E.1 Implementation in relay-runtime

1. **Host/render layer catches errors:**
   - Parse errors: returned as `Err` from `run_hook_with_host`; host displays error UI
   - Runtime errors: same — host receives error, renders error state
   - **Error VNode:** When an error occurs, relay-runtime can optionally return a standard error VNode (e.g. `{ error: true, message, file?, line? }`) instead of throwing, so host can render a consistent error UI

2. **Host responsibility:**
   - Web: `run_hook_with_host` wraps in try/catch; on error, render error div with message, file, line
   - Android: Same — `MainActivity` / `HookRenderer` catches, shows `TextView` with error details
   - No JS-based ErrorBoundary class — errors never escape to React tree; host handles at boundary

3. **Minimal error UI contract:**
   - `{ "type": "error", "message": string, "file"?: string, "line"?: number }` in VNode or as separate error result
   - Hosts render a simple error box (icon, message, optional stack)

#### E.2 Remove errorBoundary.jsx from apps

| Location | Action |
|----------|--------|
| `relay-client-web/public/hooks/errorBoundary.jsx` | Delete |
| `relay-runtime/android-demo/.../assets/hooks/errorBoundary.jsx` | Delete (via sync) |
| `hook-transpiler/tests/.../errorBoundary.jsx` | Delete |
| `relay-client-web/.../app.jsx` | Remove `import ErrorBoundary`, remove `<ErrorBoundary>` wrapper |
| `relay-client-web/.../RepoBrowser.tsx` | Remove `ErrorBoundary` from `@clevertree/hook-transpiler`; use host-level error handling |
| `hook-transpiler` | Remove ErrorBoundary export if no longer used externally |

---

### Phase F: HookRenderer (Core Component) ✅ Done

**Goal:** HookRenderer is a core component of relay-runtime. It renders local or remote hook files using the runtime. Both web and Android test apps use HookRenderer as the main rendering component. HookRenderer can be nested inside hooks to render remote peer content ("remote browser").

**New approach (vs. hook-transpiler):**
- **Single execution engine:** relay-runtime (WASM on web, native on Android) — no transpiler, no JSC
- **Simple contract:** `hookPath` (local) or `host` + `hookPath` (remote). Fetch if remote; run with runtime; render VNode
- **Composable:** A hook can render `<HookRenderer host="https://peer.example.com" />` to embed remote content

#### F.1 HookRenderer API (unified)

```ts
interface HookRendererProps {
  /** Local: path in module map (e.g. "hooks/app.jsx"). Remote: omit and use host. */
  hookPath?: string
  /** Remote: base URL to fetch hook from (OPTIONS for discovery, then fetch hookPath) */
  host?: string
  /** Optional theme override */
  themeYaml?: string
  /** Callbacks */
  onError?: (msg: string) => void
  onReady?: () => void
}
```

- **Local mode:** `hookPath` only. Uses existing module map / assets. No fetch.
- **Remote mode:** `host` (required). Fetches hook from `host + hookPath` (hookPath from OPTIONS or default). Adds fetched source to module map for that run; runs with relay-runtime.

#### F.2 Web implementation ✅ Done

- **renderHook(container, { hookPath, host?, baseUrl? })** in `relay-runtime-web.js`: Local mode fetches from baseUrl; remote mode does OPTIONS to host for path, then fetches source. Uses `run_hook_with_host`, `vnodeToDom`, and `doRender` for re-renders.
- **relay-hook-renderer:** `vnodeToDom` creates a container and calls `renderHook` for any `<relay-hook-renderer>` VNode. Supports both local (hookPath + baseUrl) and remote (host) modes.
- **Caveat:** Single global run state — nested remote hooks overwrite parent state.

#### F.3 Android implementation

1. **View:** `HookRenderer` extends `FrameLayout`:
   - Holds a `RelayRuntime` (JNI) reference
   - **Local:** Uses assets/module map from activity
   - **Remote:** OkHttp `fetch(host)` OPTIONS → get path; `fetch(host + path)` → source; pass to `runHookWithHost`
   - Renders VNode via `VNodeRenderer` (existing)
   - On `consume_render_requested`, re-run and re-render
   - Catches errors, shows error `TextView`

2. **Nested:** When VNode has a `relay-hook-renderer` (or similar) child with `host` prop, the parent `VNodeRenderer` creates a child `HookRenderer` View, passing `host`. The child does its own fetch/run/render cycle.

#### F.4 Location and ownership

| Artifact | Repo | Notes |
|----------|------|-------|
| **HookRenderer (web)** | relay-runtime | New `packages/web` or `examples/wasm-test/` React component. Uses relay-runtime WASM. |
| **HookRenderer (Android)** | relay-runtime | `android-demo/.../HookRenderer.kt` or `android/` library. Uses relay-runtime JNI. |
| **VNodeRenderer** | relay-runtime | Existing; HookRenderer uses it for VNode→View. |
| **Test app root** | relay-client-web, relay-runtime android-demo | Both load `<HookRenderer hookPath="hooks/app.jsx" />` as root. Single hardcoded entry; no manifest. |

#### F.5 Migration from hook-transpiler

- **relay-client-web:** Replace `ThemedHookRenderer` / `RepoBrowser` with `HookRenderer` from relay-runtime when migrating to relay-runtime as sole engine
- **relay-client-android:** Replace hook-transpiler's `HookRenderer` with relay-runtime's `HookRenderer` when migrating
- **peerContent.jsx:** Uses `<HookRenderer host={host} />` — same API; implementation comes from relay-runtime

---

## 5. Rendering Accuracy

### 5.1 Parity Matrix

| Feature | Web (WASM) | Android | Windows | macOS |
|---------|------------|---------|---------|-------|
| VNode structure | ✅ | Target | Target | Target |
| Element props (className, style) | ✅ | Map to View attributes | Skia + themed-styler native | Same |
| Event handlers (onClick, etc.) | ✅ | setOnClickListener | Hit-test → invoke_handler | Same |
| Theme / themed-styler | ✅ 100% CSS | android_base_styles | desktop_base_styles (new) | Same |
| Flex / positioning | ✅ | Partial | Target (priority) | Same |
| Tailwind classes | ✅ | ✅ | ✅ | Same |
| Patches (incremental) | Full replace today | Use patches for updates | Target | Same |
| Fonts | Browser | System | Bundled common set | Same |

### 5.2 Accuracy Tests

- Reuse existing fixtures (`tests/fixtures/client/`, `mini_ok.jsx`)
- Add **platform-specific snapshot tests**: run hook → VNode JSON → compare to golden file
- Android: instrumented test that runs hook, asserts view hierarchy structure

---

## 6. File and Directory Layout

```
relay-runtime/
├── Cargo.toml                    # Add android feature, ffi/native targets
├── src/
│   ├── lib.rs
│   ├── wasm_api.rs               # [cfg(wasm32)] — existing
│   ├── android_jni.rs            # [cfg(target_os = "android")] — NEW
│   ├── ffi.rs                    # [cfg(not(wasm32))] — NEW, C API for Win/macOS
│   └── ...
├── android/                      # NEW — Android library
│   ├── build.gradle
│   ├── jniLibs/
│   │   ├── arm64-v8a/
│   │   ├── armeabi-v7a/
│   │   ├── x86/
│   │   └── x86_64/
│   └── src/main/java/...         # Optional: Kotlin Host shim
├── relay-runtime-desktop/      # NEW — Skia renderer (or separate crate)
│   ├── Cargo.toml              # winit, skia-safe
│   └── src/
│       ├── lib.rs
│       ├── layout.rs           # Flex/positioning from themed-styler output
│       ├── paint.rs            # Skia draw calls
│       └── ...
├── android-demo/                # Minimal APK — EXISTS
│   ├── src/main/
│   │   ├── assets/hooks/       # Synced from relay-client-web (no errorBoundary.jsx after Phase E)
│   │   └── java/.../demo/
│   │       ├── MainActivity.kt # Uses HookRenderer as root (Phase F)
│   │       ├── VNodeRenderer.kt
│   │       └── HookRenderer.kt # Phase F — local + remote, nested
│   └── build.gradle
├── scripts/
│   ├── test-wasm.sh              # Existing
│   ├── build-android.sh          # NEW
│   └── build-native.sh           # NEW (Windows, macOS)
└── docs/
    └── CROSS_PLATFORM_OS_SUPPORT_PLAN.md  # This doc
```

---

## 7. Implementation Order

| Step | Task | Est. Effort |
|------|------|-------------|
| 0a | **Error boundary (built-in)** — Add to relay-runtime; catch parse/runtime errors, render error UI | 0.5 day |
| 0b | **Remove errorBoundary.jsx** — Delete from relay-client-web, hook-transpiler tests, relay-runtime assets; update app.jsx imports | 0.5 day |
| 0c | **HookRenderer (relay-runtime core)** — Web + Android; local/remote; nested for remote browse | 2–3 days |
| 1 | Add `android` feature, JNI deps, `android_jni.rs` with `run_hook`, `invoke_handler`, `consume_render_requested` | 1–2 days |
| 2 | Cross-compile for Android ABIs, add `android/jniLibs`, Gradle module | 0.5 day |
| 3 | Create minimal APK project, Kotlin Host (VNode→View), load hooks from assets | 2–3 days |
| 4 | Demo: all-hooks page (web), extend wasm-test with multiple hooks | 0.5 day |
| 5 | themed-styler: add `desktop_base_styles` / `native_styles` — CSS values → computed struct for Skia | 1–2 days |
| 6 | C FFI layer for Windows/macOS (`relay_run_hook`, etc.) | 1 day |
| 7 | relay-runtime-desktop: winit + Skia, layout engine, VNode→paint, hit-test, handler wiring | 3–5 days |
| 8 | Bundled fonts: common set (Inter, Roboto, system fallbacks) | 0.5 day |
| 9 | Platform parity tests (snapshot, instrumented) | 1 day |
| 10 | Extend Host for native components (audio, video, canvas) — future | 2+ days |
| 11 | Optional: npm React as a **native component** (not embed/`parked` dep) — deferred | TBD |

---

## 8. Dependencies Summary

| Workspace | Role |
|-----------|------|
| **relay-runtime** | Core; adds Android JNI, C FFI |
| **themed-styler** | Single styling source; add `desktop`/`native` output for Skia renderer |
| **relay-runtime-desktop** | New crate or in-tree: winit + Skia, layout, VNode→paint |
| **relay-client-android** | Future consumer; minimal APK is standalone first |
| **relay-client-web** | Uses WASM; no changes |

---

## 9. Outstanding Issues (to resolve over time)

| Issue | Description | Approach |
|-------|-------------|----------|
| **Responsive breakpoints** | theme.yaml may define breakpoints; Android/desktop need to map to viewport/screen size | Pass viewport dimensions to themed-styler; media-query-like resolution per platform |
| **DPI / density** | Android has density; desktop has scale factor (e.g. 125%, 150%) | themed-styler's `dp_to_px`, `parse_and_convert_to_px` already handle density; ensure desktop receives scale factor |
| **Z-index / stacking** | Web has natural stacking context; native needs explicit paint order | Layout engine maintains tree order; optional z-index sort before paint |
| **Shadows / borders** | CSS box-shadow, border-radius — Skia supports; Android has elevation | Map to platform equivalents; some features may degrade gracefully |
| **Transforms** | translate, scale, rotate | Skia supports; include in desktop style struct if themed-styler emits |
| **Image loading** | `<img src="...">` | Host must fetch and provide bitmap; relay-runtime produces VNode with src; host resolves |
| **Accessibility** | Screen readers, focus order | Defer; add semantic attributes to VNode, host maps to platform a11y APIs |
| **RTL** | Right-to-left layouts | themed-styler and layout engine need `direction` awareness; lower priority |
| **Dark/light theme** | themed-styler has themes; switch via `set_theme_and_render` | All platforms already consume theme; ensure desktop respects same flow |

---

## 10. Native Components Roadmap (Future)

To support audio/video/gaming without rerender storms:

1. **Host trait extension:** `create_native_component(tag, props, children)` returning an opaque handle
2. **Tag allowlist:** `video`, `audio`, `canvas`, `GameView` → host creates native view once
3. **Control flow:** Handlers (play, pause, etc.) call `invoke_handler`; no full tree re-run for media state
4. **Documentation:** Recommend authors use native components for high-frequency updates

---

## 11. Plan Update Summary (Latest)

| Decision | Details |
|----------|---------|
| **Error boundary in relay-runtime** | Build error handling into relay-runtime. Host catches parse/runtime errors and renders error UI. No JS `ErrorBoundary` component. |
| **Remove errorBoundary.jsx** | Delete from relay-client-web, hook-transpiler tests, relay-runtime assets. Update app.jsx and RepoBrowser to drop ErrorBoundary wrapper. |
| **HookRenderer as relay-runtime core** | New implementation for web + Android. Uses relay-runtime (WASM/native) only. Renders local (module map) or remote (fetch from host). Nested for remote browsing. |
| **Test apps use HookRenderer as root** | Both web and Android test apps load `<HookRenderer hookPath="hooks/app.jsx" />` as the main component. Nested `<HookRenderer host="..." />` for peer content. |
| **Fresh approach** | Do not port hook-transpiler's HookRenderer. New design: relay-runtime as sole engine, simple local/remote API, composable nesting. |

---

## 12. References

- [PLAN.md](PLAN.md) — Architecture, re-render phases
- [IMPLEMENTATION_PHASES.md](IMPLEMENTATION_PHASES.md) — Phase 5 DOM/Native
- [WEB_WASM_HOST.md](WEB_WASM_HOST.md) — Web host contract
- hook-transpiler `android_jni.rs` — JNI pattern reference
- themed-styler `android.rs`, `Cargo.toml` — Android feature, JNI, CSS→native conversion
- themed-styler (to extend) — Add `desktop_base_styles` / `native_styles` for Skia renderer
