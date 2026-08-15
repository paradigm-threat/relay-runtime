# Next steps (relay-runtime & Relay clients)

Ordered work aligned with [POLICY.md](POLICY.md): **no legacy**, **no hook transpile to JS**, **relay-runtime** (with **themed-styler compiled in** via path dependency) as the only hook engine.

## Priority phases

| Phase | Goal | Status |
|-------|------|--------|
| **A0** | Android SDK + device, finish `relay-runtime` Android, demo APK vs wasm-test (same hooks; web is gold) | **Now** — [ANDROID_WEB_TEST.md](ANDROID_WEB_TEST.md) |
| **A** | Library UI on web + Android product client against **fan-edits** | After A0 — see [fan-edits/docs/RELAY_UI_TEST_PLAN.md](../../fan-edits/docs/RELAY_UI_TEST_PLAN.md) |
| **B** | Native video playback (web + Android; not Windows native) | Deferred |
| **C** | Timeline editor / `.reel` | Deferred |
| **Desktop native** | Windows/macOS Skia UI | **Skipped** — use desktop web |

---

## relay-runtime (this repo)

1. **Parser/runtime parity** — `relay-template/hooks/client/*` (historical phases: [archive/IMPLEMENTATION_PHASES.md](archive/IMPLEMENTATION_PHASES.md)).
2. **Cross-origin fetch** ✅ — `relayClientBaseUrl` in hooks manifest resolves relative `fetch()` on WASM/Android.
3. **Stable render loop** ✅ — Web glue runs up to 16 passes on `consume_render_requested()`; desktop/Android mirror this.
4. **DOM / native host commit** — Versioned host API; [WEB_WASM_HOST.md](WEB_WASM_HOST.md).
5. **Hooks** — `useEffect` dependency scheduling, `useCallback` memoization (needed for relay-template; fan-edits works with simplified semantics).
6. **Integration tests** — Enable ignored Layout test; relay-template plugin hooks.
7. **Docs** — Keep POLICY, CLEVERSCRIPT, and ANDROID_WEB_TEST in sync. Historical PLAN/phases live under `docs/archive/`.

---

## relay-client-web

1. **Thin WASM shell** ✅ — `ThemedHookRenderer`, preset remote hook mode.
2. **fan-edits smoke** ✅ — `npm run test:e2e:fan-edits`, `npm run serve:fan-edits`.
3. **Remove** stale hook-transpiler README sections (ongoing).

---

## relay-client-desktop

1. **fan-edits MVP** ✅ — `--repo ~/dev/fan-edits`, `--verify`, integration tests.

---

## relay-client-android

Full RelayClient (not the runtime test APK). Uses compiled **Rust** `librelay_runtime.so`.

1. **Do not start here.** Prove text + styles on `relay-runtime/android-demo` vs `examples/wasm-test` first ([ANDROID_WEB_TEST.md](ANDROID_WEB_TEST.md)).
2. **relay-runtime JNI** ✅ — `HookRenderer`, `RelayRuntimeHostHttp`, fan-edits remote gateway.
3. Re-sync `RelayRuntime.kt` + `jniLibs` after `relay-runtime/scripts/build-android.sh`.

---

## relay-template (language parity, not UI MVP)

- Full movie app hooks (plugins, Layout) — separate from fan-edits Phase A.

---

## hook-transpiler

- **Deprecated for Relay** — do not extend for hook execution.
