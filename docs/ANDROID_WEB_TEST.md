# Android ↔ web tester (relay-runtime)

**Canonical tester doc for this crate.** Keep it current: if you change Android/web test behavior, update **Current state**, **Next steps**, and the **Progress log** in the same change (see [Agent instructions](#agent-instructions)).

Last reviewed: **2026-08-15**.

This is the build-and-compare loop. Do **not** start in the product Android client. Runtime first, same hooks on web and Android, **web is the visual standard**.

---

## Current state (2026-08-15)

| Piece | Status | Notes |
|-------|--------|-------|
| **Engine** | Shipped | Rust CleverScript in this crate. Android loads **`librelay_runtime.so`** (JNI), not npm. **themed-styler** is compiled in. |
| **Style contract** | Shipped | Runtime resolves `className` + theme → `props.style`. Kotlin **applies** only; it must not parse Tailwind. |
| **In-tree test APK** | Exists | `android-demo/` — `./scripts/build-install-demo.sh` installs `com.clevertree.relayruntime.demo`. Entry `hooks/app.jsx`. JNI, `HookRenderer`, `VNodeRenderer` (text, color, padding, radius, flex via LinearLayout). |
| **Web test app** | Exists | `examples/wasm-test/` — `./scripts/test-wasm.sh` → `http://localhost:8765/examples/wasm-test/`. |
| **Hook sync** | Incomplete | Demo JSX lives in both `android-demo/src/main/assets/hooks/` and `examples/wasm-test/hooks/`. `scripts/sync-hooks.sh` still copies from **relay-client-web**, which is the wrong SoT for **this** harness. Need one shared hook tree for wasm-test ↔ android-demo. |
| **Visual parity** | **Not signed off** | APK and wasm-test have not been compared section-by-section (text, theme tokens, flex, spacing). That is the current gate. |
| **Product client** | Separate | `relay-client-android` = full RelayClient; copies `.so` after `scripts/build-android.sh`. Not the test harness. |
| **Desktop native** | Skipped | No Windows/macOS from-scratch UI. Desktop = web browser (or packaged web app). |
| **iOS** | Later | Same native-prop translation as Android. |

SDK: typical operator machines have `ANDROID_HOME` / `adb`. Confirm `adb devices` before install.

---

## Repos (do not confuse)

| Tree | What it is | What it is not |
|------|------------|----------------|
| **`relay-runtime`** | Engine + **test APK** + **web test app**. Prove every runtime change here first. | Not a product Relay client. |
| **`relay-client-android`** | Full **RelayClient** (library UI, remote gateway). Rust JNI `.so`. | Not the runtime test harness. |

If the demo APK is wrong, the product app will be wrong.

---

## Testing plan (in order)

### 0. Setup

```bash
cd ~/dev/relay/relay-runtime   # or this clone
adb devices                    # phone or emulator
```

### 1. Finish runtime Android gaps

Gaps belong in this crate: `src/android_jni.rs`, `src/style_resolve.rs`, `android-demo/.../VNodeRenderer.kt`, `HookRenderer.kt`. Add a fixture or demo hook. Do not paper over them in `relay-client-android`.

### 2. Build and run the test APK

```bash
./scripts/build-install-demo.sh
# logs: adb logcat -s RelayDemo:D
```

**Pass:** text, layout, and theme styles render (not unstyled system defaults). Walk demo sections: counter / useState, style keywords, flex, headings, nested structure.

### 3. Same hooks on the web test app

```bash
./scripts/test-wasm.sh
# http://localhost:8765/examples/wasm-test/
```

Web and Android must run the **same client hooks**. Prefer one source (`examples/wasm-test/hooks/`) and keep `android-demo/src/main/assets/hooks/` in sync. Host glue may differ; **hooks must not**.

### 4. Compare Android to web

**Web is the default standard** (CSS / Tailwind subset / `theme.yaml`). We do not support all of CSS. Android maps resolved styles to native props (`setPadding`, `setTextColor`, `setBackgroundColor`, flex, …). If Android disagrees with web, **fix the runtime or the Kotlin apply map**, not the hook — unless the hook used a web-only API (`document`, `html.dark`).

Compare: same section tree and copy; typography; spacing; surfaces; flex/gap; theme tokens.

### 5. Only then: product client

Rebuild JNI, re-sync `jniLibs` + `RelayRuntime.kt` into **`relay-client-android`**. That app talks to a live gateway; it is not a substitute for the in-tree APK.

---

## Next steps

Update this list when an item lands (date + one-line result). Do not leave stale “Now” items.

| # | Item | Status |
|---|------|--------|
| 1 | Confirm device/emulator; `build-install-demo.sh` launches without crash | **Now** |
| 2 | Screenshot/walk wasm-test vs APK for `hooks/app.jsx` sections (text + styles) | **Now** |
| 3 | Unify hook SoT: wasm-test hooks ↔ android-demo assets (stop using product `relay-client-web` as the demo SoT) | Open |
| 4 | Fix mismatches in runtime or `VNodeRenderer` apply map; add a regression hook if needed | Open |
| 5 | Document remaining CSS-subset gaps (unsupported on native by design) | Open |
| 6 | Re-sync `.so` into `relay-client-android` only after demo ≈ web | Blocked on 2–4 |
| 7 | iOS native-prop host (same contract) | Later |
| 8 | Windows/macOS native UI | **Won’t do** — desktop web |

---

## Progress log

Append dated bullets when testing moves. Newest first.

- **2026-08-15** — Tester doc created. APK + wasm-test exist; visual parity not signed off; hook sync still points at relay-client-web.

---

## Out of scope

- Windows / macOS **native** UI. Use desktop **web**.
- A separate Android **repo** for the test APK — it stays in **`relay-runtime`**.
- Full browser CSS. Native gets the themed-styler subset.

## Related

- [CROSS_PLATFORM_OS_SUPPORT_PLAN.md](CROSS_PLATFORM_OS_SUPPORT_PLAN.md)
- [STYLE_RESOLUTION.md](STYLE_RESOLUTION.md)
- [WEB_WASM_HOST.md](WEB_WASM_HOST.md)
- [NEXT_STEPS.md](NEXT_STEPS.md)
- Product shell: `../relay-client-android/README.md`

---

## Agent instructions

**Any agent or LLM working on Android, JNI, `android-demo`, wasm-test hooks, or visual parity MUST update this file in the same change** (do not wait for a later “docs pass”).

When you:

- land or close a **Next steps** row — set Status (`Done YYYY-MM-DD` / `Now` / `Blocked: …`) and add a **Progress log** bullet
- discover a **gap** (missing tag, style key, crash, hook drift) — add it to Next steps or Current state; do not only mention it in chat
- **sign off** a demo section vs web — say which sections matched; bump Last reviewed
- change **commands**, package id, entry hook, or log tag — fix the Testing plan snippets here
- finish hook SoT unification — rewrite the Hook sync row so it is no longer “Incomplete”

Do **not** mark visual parity signed off unless Android was compared to wasm-test on a device/emulator for the shared `app.jsx` (or successor) sections. Do **not** treat `relay-client-android` as proof of runtime correctness.

Keep the doc **generic** (no named testers). Keep Current state a table, Next steps a table, Progress log dated.
