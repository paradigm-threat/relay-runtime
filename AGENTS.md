# Agent instructions (relay-runtime)

Run tests: **`cargo test`**.

**Android ↔ web tester:** [`docs/ANDROID_WEB_TEST.md`](docs/ANDROID_WEB_TEST.md). In-tree APK (`android-demo/`) and wasm-test first; product client is `relay-client-android`. Web is the visual standard.

When you change Android testing, JNI, demo hooks, or visual parity, **update `docs/ANDROID_WEB_TEST.md` in the same change** (Current state, Next steps, Progress log). See that file’s **Agent instructions**.
