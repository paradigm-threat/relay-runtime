# Park-authored native components

**Normative** contract for complex UI that should **not** be implemented as CleverScript trees.

Relay **supports** park- (or app-) authored natives the way React Native supports per-OS files: one CleverScript import, host picks the platform implementation. Product widgets live in the **park tip**, not inside Relay distributions. Builtins such as `@clevertree/video` `VideoPlayer` remain host-shipped primitives parks may compose **inside** a park host file.

## CleverScript vs park native

| CleverScript (tip hooks) | Park native (per-OS host files) |
|--------------------------|----------------------------------|
| Route / path selection, SSR shell shape | Complex reusable widgets (affirmation, marquee, media chrome, maps, editors) |
| App/session state, ownership flags as data | High-churn interaction, animation, media playback chrome |
| Fetch / query / index walks, wiring props into natives | OS-specific look & persistence (`localStorage` vs SharedPreferences) |
| Thin composition: `<Affirmation … onConfirm={…} />` | Host owns inner DOM/views; may use modern JS/CSS; no CS `setState` per frame |

**Use a park native when** the UI is reusable across screens or OS surfaces, needs continuous animation or media, or repeatedly fails soft-hydrate / CleverScript constraints (no `document`, expensive full-hook re-renders, SSR module drops).

**Keep CleverScript for** orchestration — state, routing, general wiring, querying — not pixel-level controls.

See also [`RELAY_CLEVERSCRIPT_REACT_RENDERING.md`](../../relay-gateway/docs/RELAY_CLEVERSCRIPT_REACT_RENDERING.md) and this runtime’s [`CLEVERSCRIPT.md`](CLEVERSCRIPT.md).

## File naming (RN-style)

Under the park tip, e.g. `hooks/client/native/`:

| File | Role |
|------|------|
| `Affirmation.js` | CleverScript stub: factory returns a VNode with a custom tag / `data-relay-park-native` + props (no `document`) |
| `Affirmation.host.js` (or `Affirmation.web.js`) | Web host builder: modern JS/CSS, may use `document` / `localStorage` — **never** imported by CleverScript |
| `Affirmation.android.js` | Android host stub or bridge until a Kotlin view lands |
| `registry.host.js` | Registers tag → builder with the web host (optional; tip inject may load hosts directly) |

Module resolution prefers `Foo.<platform>.js` then `Foo.js` (see relay-runtime host resolve).

## Registration (web)

1. Park ships `hooks/client/native/registry.web.js` (or path listed in embed / web-assets config).
2. Host script calls the published API, e.g. `globalThis.__relayRegisterParkNative(tag, buildFn)`.
3. On `vnodeToDom`, if `tag` is registered → call park builder; else builtins (`relay-video-player`, …); else plain `createElement`.

Builder signature (web):

```js
function build(props, invokeHandler, onAfterInvoke) {
  // props: plain object; function props already HandlerId numbers
  // return an HTMLElement
}
```

Events: call `invokeHandler(handlerId)` then optional `onAfterInvoke()` (same pattern as `buildRelayVideoPlayer`).

## Testing

| Layer | What |
|-------|------|
| Layer 1 | CleverScript stub emits expected tag + props (`CLEVERSCRIPT_COMPONENT_TESTING.md`) |
| Host isolation | Playwright / vitest against `*.web.js` builders alone (no full park boot) |
| Park smoke | CDP / fleet after publish |

## Anti-patterns

- Shipping park product UI inside `relay-runtime` / gateway distributions
- Importing `@clevertree/video` from CleverScript trees that must SSR when the graph drops that module — compose video **inside** the park host builder instead
- Driving animation or checkbox UX with CleverScript `setState` + soft-hydrate remounts
