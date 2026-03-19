/**
 * relay-runtime web integration
 *
 * Provides bootstrap() and vnodeToDom for zero-config web integration.
 * All logic (fetch, run, render) lives here—use only minimal browser glue to load.
 *
 * @example
 *   <script type="module">
 *     import { bootstrap } from 'relay-runtime';
 *     bootstrap();
 *   </script>
 */

import init, {
  run_hook_with_host,
  get_injected_css,
  get_themes,
  set_theme_and_render,
} from './relay_runtime.js';

let _styleId = 'themed-styler-output';

/**
 * Convert VNode JSON to DOM nodes.
 * Handles Element (VNode format), type: "element" (Value format), Text, and arrays.
 *
 * @param {object} v - VNode or Value
 * @returns {DocumentFragment|HTMLElement|Text|null}
 */
export function vnodeToDom(v) {
  if (!v) return null;
  if (v.Empty !== undefined) return null; // VNode::Empty
  if (v.Text !== undefined) return document.createTextNode(v.Text);
  if (v.Element) {
    const { tag, props, children } = v.Element;
    if (tag === 'fragment') {
      const frag = document.createDocumentFragment();
      for (const c of children || []) {
        const child = vnodeToDom(c);
        if (child) frag.appendChild(child);
      }
      return frag;
    }
    const el = document.createElement(tag);
        if (props) {
      for (const [k, val] of Object.entries(props)) {
        const str =
          val && typeof val === 'object' && val.type === 'String' && val.value !== undefined
            ? val.value
            : typeof val === 'string'
              ? val
              : null;
        if (str != null) {
          if (k === 'className') el.className = str;
          else if (k.startsWith('data') && k.length > 4) {
            const attr = 'data-' + k.slice(4).replace(/([A-Z])/g, '-$1').toLowerCase().replace(/^-/, '');
            el.setAttribute(attr, str);
          } else el.setAttribute(k, str);
        }
      }
    }
    if (children && Array.isArray(children)) {
      for (const c of children) {
        const child = vnodeToDom(c);
        if (child) el.appendChild(child);
      }
    }
    return el;
  }
  if (v.type === 'element') {
    const el = document.createElement(v.tag === 'fragment' ? 'div' : v.tag);
    if (v.props) {
      for (const [k, val] of Object.entries(v.props)) {
        const str =
          val && typeof val === 'object' && val.type === 'String' && val.value !== undefined
            ? val.value
            : typeof val === 'string'
              ? val
              : null;
        if (str != null) {
          if (k === 'className') el.className = str;
          else if (k.startsWith('data') && k.length > 4) {
            const attr = 'data-' + k.slice(4).replace(/([A-Z])/g, '-$1').toLowerCase().replace(/^-/, '');
            el.setAttribute(attr, str);
          } else el.setAttribute(k, str);
        }
      }
    }
    if (v.children && Array.isArray(v.children)) {
      for (const c of v.children) {
        const child = vnodeToDom(c);
        if (child) el.appendChild(child);
      }
    }
    return el;
  }
  if (Array.isArray(v)) {
    const frag = document.createDocumentFragment();
    for (const c of v) {
      const child = vnodeToDom(c);
      if (child) frag.appendChild(child);
    }
    return frag;
  }
  return null;
}

/**
 * Default baseUrl: directory containing theme.yaml, hooks.json, etc.
 * Resolve relative to this script so it works regardless of document URL.
 */
function getDefaultBaseUrl() {
  try {
    return new URL('../', import.meta.url).href;
  } catch {
    return '.';
  }
}

/**
 * Default paths relative to baseUrl.
 */
const DEFAULTS = {
  rootId: 'root',
  styleId: 'themed-styler-output',
  themePath: 'theme.yaml',
  hooksPath: 'hooks.json',
  appPath: 'hooks/app.jsx',
  get baseUrl() {
    return getDefaultBaseUrl();
  },
};

/**
 * Resolve a path against baseUrl.
 * @param {string} baseUrl
 * @param {string} path
 * @returns {string}
 */
function resolveUrl(baseUrl, path) {
  if (!baseUrl || baseUrl === '.') return path;
  const base = baseUrl.endsWith('/') ? baseUrl : baseUrl + '/';
  return path.startsWith('./') ? base + path.slice(2) : base + path;
}

/**
 * Bootstrap relay-runtime: load WASM, fetch config and hooks, run app, inject CSS, render to DOM.
 *
 * @param {object} [options]
 * @param {string} [options.rootId='root'] - DOM id for mounting
 * @param {string} [options.styleId='themed-styler-output'] - DOM id for injected CSS
 * @param {string} [options.themePath='theme.yaml']
 * @param {string} [options.hooksPath='hooks.json']
 * @param {string} [options.appPath='hooks/app.jsx']
 * @param {string} [options.baseUrl='.'] - Base URL for resolving paths (relative to page)
 * @param {string|URL} [options.wasmUrl] - Override WASM module URL (default: next to relay_runtime.js)
 * @returns {Promise<void>}
 */
export async function bootstrap(options = {}) {
  const opts = { ...DEFAULTS, ...options };
  const { rootId, styleId, themePath, hooksPath, appPath, baseUrl, wasmUrl } = opts;

  const root = document.getElementById(rootId);
  if (!root) {
    console.error('[relay-runtime] Root element not found:', rootId);
    return;
  }

  try {
    await init(wasmUrl);
    _styleId = styleId;

    const themeUrl = resolveUrl(baseUrl, themePath);
    const hooksUrl = resolveUrl(baseUrl, hooksPath);
    const appUrl = resolveUrl(baseUrl, appPath);
    const cacheBust = '?t=' + Date.now();

    const [themeResp, hooksResp, appResp] = await Promise.all([
      fetch(themeUrl + cacheBust),
      fetch(hooksUrl + cacheBust),
      fetch(appUrl + cacheBust),
    ]);

    if (!themeResp.ok) {
      console.warn('[relay-runtime] theme fetch failed:', themeResp.status, themeUrl);
    }
    if (!hooksResp.ok) {
      console.warn('[relay-runtime] hooks fetch failed:', hooksResp.status, hooksUrl);
    }
    if (!appResp.ok) {
      console.warn('[relay-runtime] app fetch failed:', appResp.status, appUrl);
    }

    const themeYaml = themeResp.ok ? await themeResp.text() : '';
    const manifest = hooksResp.ok ? await hooksResp.json() : { hooks: [] };
    const appSource = appResp.ok ? await appResp.text() : '';

    const moduleMap = {};
    for (const h of manifest.hooks || []) {
      const url = resolveUrl(baseUrl, h.path) + cacheBust;
      const resp = await fetch(url);
      if (resp.ok) {
        moduleMap[h.path] = await resp.text();
      } else {
        console.warn('[relay-runtime] hook fetch failed:', resp.status, h.path, url);
      }
    }

    const hookCount = (manifest.hooks || []).length;
    const loadedCount = Object.keys(moduleMap).length;
    if (hookCount === 0) {
      console.warn('[relay-runtime] hooks manifest has no hooks');
    } else if (loadedCount < hookCount) {
      console.warn('[relay-runtime] loaded', loadedCount, 'of', hookCount, 'hooks; baseUrl:', baseUrl);
    }

    // Debug: log what we're passing when troubleshooting empty render
    if (hookCount === 0 || loadedCount === 0 || !appSource) {
      console.warn('[relay-runtime] bootstrap inputs:', {
        themeLen: themeYaml.length,
        hookCount,
        loadedCount,
        appLen: appSource.length,
        modulePaths: Object.keys(moduleMap),
        baseUrl,
      });
    }

    const vnodeJson = run_hook_with_host(
      appSource,
      JSON.stringify(moduleMap),
      themeYaml,
      JSON.stringify(manifest),
      appPath || ''
    );

    const css = get_injected_css();
    const styleEl = document.getElementById(styleId);
    if (styleEl) styleEl.textContent = css;

    root.innerHTML = '';
    const vnode = JSON.parse(vnodeJson);
    const el = vnodeToDom(vnode);
    if (el) {
      root.appendChild(el);
    } else {
      const empty = document.createElement('p');
      empty.className = 'relay-empty';
      empty.textContent = 'Rendered (no content). Check that hooks manifest and theme are loaded.';
      root.appendChild(empty);
      if (typeof console !== 'undefined' && console.debug) {
        console.debug('[relay-runtime] vnode was empty:', vnode);
      }
    }

    // Event delegation for theme toggle (buttons with dataTheme -> data-theme)
    root.addEventListener('click', (e) => {
      const theme = e.target?.dataset?.theme;
      if (theme) setCurrentTheme(theme);
    });
  } catch (e) {
    root.textContent = '';
    root.className = 'error';
    let msg = e?.message ?? e?.toString?.() ?? String(e);
    // Parse errors may be JSON {message, line, column, file?, source_line?} for advanced UI
    try {
      const parsed = JSON.parse(msg);
      if (parsed && typeof parsed.message === 'string') {
        const loc = [parsed.file, parsed.line, parsed.column].filter(Boolean).join(':');
        msg = loc ? `${loc}: ${parsed.message}` : parsed.message;
        if (parsed.source_line) {
          msg += `\n  ${parsed.source_line}\n  ${' '.repeat((parsed.column || 1) - 1)}^`;
        }
      }
    } catch (_) { /* not JSON, use as-is */ }
    root.textContent = `Init error: ${msg || 'Unknown error'}`;
    root.style.whiteSpace = 'pre-wrap';
    console.error('[relay-runtime] Init error:', e);
  }
}

/**
 * Get available theme names. Call after bootstrap().
 * @returns {string[]} Theme keys (e.g. ['default', 'light', 'dark'])
 */
export function getThemes() {
  try {
    const json = get_themes();
    const arr = JSON.parse(json || '[]');
    return Array.isArray(arr) ? arr.map((t) => (t && t.key ? t.key : String(t))) : [];
  } catch {
    return [];
  }
}

/**
 * Set current theme and update styles. Use with theme toggle buttons (data-theme="...").
 * @param {string} name - Theme name (e.g. 'light', 'dark', 'default')
 */
export function setCurrentTheme(name) {
  const css = set_theme_and_render(name);
  const styleEl = document.getElementById(_styleId);
  if (styleEl) styleEl.textContent = css;
}
