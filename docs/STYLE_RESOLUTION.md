# Style Resolution

*Last updated: 2026-08-02 (ET)*

## Overview

**themed-styler** is the single source for theme tokens and class → style maps. How the host **applies** those styles differs by platform:

| Platform | Apply model | Theme toggle without CleverScript remount |
|----------|-------------|-------------------------------------------|
| **Web** | Keep `className`; inject stylesheet from `css_for_web_system()` (light + dark + `prefers-color-scheme` + `html.dark` / `html.light`) | Flip `document.documentElement` class (and optional tip attr such as `data-pt-theme`). **Do not** call `setCurrentTheme` for chrome. |
| **Android / iOS / Desktop** | Flat **inline** `style` per node (`android_styles_for` / native path). No CSS cascade. | **Today:** `nativeSetThemeAndRender(name)` re-resolves the **stored unresolved VNode** under the new theme and returns JSON for one native apply — **no hook re-run**. **Future (not implemented):** dual style maps on each node so the host swaps maps with zero re-resolve. |

Web dual CSS is the park pattern (`ThemeToggle` + `html.light`/`html.dark`). Android tips must not copy `html.dark` selectors; call the JNI theme API (or a future dual-map host helper) instead.

## Theme loading (`@clevertree/style`)

Hooks own theme YAML. Wrap UI in **`StyleContext`** from `@clevertree/style`:

```jsx
import StyleContext from '@clevertree/style'

export default function MyHook() {
  return (
    <StyleContext src="hooks/client/theme.yaml">
      <div className="my-app">…</div>
    </StyleContext>
  )
}
```

- **`src`** — repo-relative path to `theme.yaml` (fetched via host `fetch()`).
- Children render only after YAML is validated and stored in the runtime session.
- Hosts pass **empty** `theme_yaml` to `run_hook_with_host`; resolution uses the session store.
- Named themes in YAML should include both **`light`** and **`dark`** when chrome toggle is required (enables `css_for_web_system` dual sheet).

## Merge Order (Lowest → Highest Priority)

1. **Tag defaults** — e.g. `div` gets `width: 100%` (web) or `match_parent` (Android)
2. **Theme selectors** — e.g. `.bg-primary` from theme.yaml (active theme on native; both themes in web dual sheet)
3. **Tailwind dynamic utilities** — e.g. `p-4`, `flex`, `gap-4`
4. **Inline `style` prop** — wins over all

Native: no cascade tree — later wins into one flat map. Web stylesheet: cascade via selectors / `html.dark` prefixes.

## Light / dark — contracts

### Web (shipped)

1. Runtime emits dual CSS via `css_for_web_system` after style resolve.
2. Tip chrome flips `html` class to `light` or `dark` (park also sets `data-pt-theme` so `syncHtmlThemeClass` does not force WASM `current_theme` light after remount).
3. Tip `navChromeCss` / icon swap may use web-only injected CSS for glass / `html.dark` icon rules — that path is **not** portable to Android.

### Android (prep — host API exists; dual-map not built)

1. Keep `themes.light` / `themes.dark` in tip `theme.yaml` (same tokens as web).
2. Appearance change: JNI **`nativeSetThemeAndRender(themeName)`** → re-resolve stored VNode → Kotlin `render(json)` once.
3. CleverScript tips should expose a thin “set appearance” that the **Android host** maps to that JNI call — **not** `document.documentElement` class flips.
4. Optional later: emit both light and dark style maps per node at first resolve so toggle is apply-only (document when implemented).

## Supported Utilities (Both Platforms)

| Category     | Classes / Examples                    |
|-------------|---------------------------------------|
| Flex        | `flex`, `flex-row`, `flex-col`, `flex-1` |
| Spacing     | `p-4`, `m-2`, `gap-4`, `space-x-4`, `space-y-4` |
| Typography  | `text-base`, `font-semibold`, `text-primary` |
| Colors      | `bg-primary`, `text-muted`, theme vars |
| Borders     | `rounded`, `border`                   |
| Sizing      | `w-full`, `h-screen`                  |

## Platform-Specific Output

| Platform  | Units   | Extra keys                    |
|----------|---------|-------------------------------|
| Web      | px, %   | stylesheet injection; `className` retained |
| Android  | dp, sp  | `androidOrientation`, `androidGravity` |

Theme colors resolve identically on both (e.g. `#3b82f6`).
