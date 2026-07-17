# Style Resolution

## Overview

All platforms use **flat inline style** resolution. No cascading, no CSS injection. Web and native behave the same: `className` + tag → resolved `style` per element.

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
- Legacy `@theme` / host-injected theme YAML is removed (pre-release cutover).

## Merge Order (Lowest → Highest Priority)

1. **Tag defaults** — e.g. `div` gets `width: 100%` (web) or `match_parent` (Android)
2. **Theme selectors** — e.g. `.bg-primary` from theme.yaml
3. **Tailwind dynamic utilities** — e.g. `p-4`, `flex`, `gap-4`
4. **Inline `style` prop** — wins over all

No cascade tree. Later wins.

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
| Web      | px, %   | `flexDirection`, `gap`        |
| Android  | dp, sp  | `androidOrientation`, `androidGravity` |

Theme colors resolve identically on both (e.g. `#3b82f6`).
