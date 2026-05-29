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
  push_message_from_host,
  invoke_handler,
  consume_render_requested,
} from './relay_runtime.js';

let _styleId = 'themed-styler-output';

/**
 * Convert VNode JSON to DOM nodes.
 * Handles Element (VNode format), type: "element" (Value format), Text, and arrays.
 *
 * @param {object} v - VNode or Value
 * @param {function(number): void} [invokeHandler] - Called for Handler props (onClick, etc.). Default: invoke_handler from WASM.
 * @param {function(): void} [onAfterInvoke] - Called after each handler; if consume_render_requested(), should re-run and re-apply.
 * @returns {DocumentFragment|HTMLElement|Text|null}
 */
export function vnodeToDom(v, invokeHandler = invoke_handler, onAfterInvoke = null) {
  if (!v) return null;
  if (v.Empty !== undefined) return null; // VNode::Empty
  if (v.Text !== undefined) return document.createTextNode(v.Text);
  if (v.Element) {
    const { tag, props, children } = v.Element;
    if (tag === 'relay-hook-renderer') {
      const host = (props?.host?.value ?? props?.host) ?? '';
      const hookPath = (props?.hookPath?.value ?? props?.hookPath) ?? 'hooks/app.jsx';
      const baseUrl = (props?.baseUrl?.value ?? props?.baseUrl) ?? undefined;
      const container = document.createElement('div');
      container.className = 'relay-hook-renderer';
      renderHook(container, { hookPath, host: host || undefined, baseUrl }).catch((e) => {
        container.textContent = `Hook error: ${e?.message ?? e}`;
        container.className = 'relay-hook-renderer error';
      });
      return container;
    }
    if (tag === 'fragment') {
      const frag = document.createDocumentFragment();
      for (const c of children || []) {
        const child = vnodeToDom(c, invokeHandler, onAfterInvoke);
        if (child) frag.appendChild(child);
      }
      return frag;
    }
    const el = document.createElement(tag);
    if (props) {
      for (const [k, val] of Object.entries(props)) {
        if (val && typeof val === 'object' && (val.type === 'Handler' || val.type === 'HandlerId') && typeof val.value === 'number') {
          const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
          const handlerId = val.value;
          el.addEventListener(eventType, () => {
            try {
              invokeHandler(handlerId);
            } catch (e) {
              console.error('[relay-runtime] Handler invocation failed:', e);
              throw e;
            }
            if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
          });
          continue;
        }
        if (k === 'style') {
          const styleObj = val && typeof val === 'object' && val.type === 'Style' ? val.value : val;
          if (styleObj && typeof styleObj === 'object') {
            for (const [pk, pv] of Object.entries(styleObj)) {
              if (pv != null && typeof pv !== 'object') {
                el.style[pk] = String(pv);
              }
            }
          }
          continue;
        }
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
      const parentStyle = props?.style?.value ?? props?.style ?? {};
      const isRow = parentStyle.flexDirection === 'row' || (!parentStyle.flexDirection && parentStyle.display === 'flex');
      const spacing = isRow ? (parentStyle.spaceX ?? parentStyle.gap) : (parentStyle.spaceY ?? parentStyle.gap);
      for (let i = 0; i < children.length; i++) {
        const child = vnodeToDom(children[i], invokeHandler, onAfterInvoke);
        if (child) {
          if (i > 0 && spacing != null && child.style) {
            const px = typeof spacing === 'number' ? spacing + 'px' : String(spacing);
            child.style[isRow ? 'marginLeft' : 'marginTop'] = px;
          }
          el.appendChild(child);
        }
      }
    }
    return el;
  }
  if (v.type === 'element') {
    const el = document.createElement(v.tag === 'fragment' ? 'div' : v.tag);
    if (v.props) {
      for (const [k, val] of Object.entries(v.props)) {
        if (val && typeof val === 'object' && (val.type === 'Handler' || val.type === 'HandlerId') && typeof val.value === 'number') {
          const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
          el.addEventListener(eventType, () => {
            try {
              invokeHandler(val.value);
            } catch (e) {
              console.error('[relay-runtime] Handler invocation failed:', e);
              throw e;
            }
            if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
          });
          continue;
        }
        if (k === 'style') {
          const styleObj = val && typeof val === 'object' && val.type === 'Style' ? val.value : val;
          if (styleObj && typeof styleObj === 'object') {
            for (const [pk, pv] of Object.entries(styleObj)) {
              if (pv != null && typeof pv !== 'object') {
                el.style[pk] = String(pv);
              }
            }
          }
          continue;
        }
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
      const parentStyle = v.props?.style?.value ?? v.props?.style ?? {};
      const isRow = parentStyle.flexDirection === 'row' || (!parentStyle.flexDirection && parentStyle.display === 'flex');
      const spacing = isRow ? (parentStyle.spaceX ?? parentStyle.gap) : (parentStyle.spaceY ?? parentStyle.gap);
      for (let i = 0; i < v.children.length; i++) {
        const child = vnodeToDom(v.children[i], invokeHandler, onAfterInvoke);
        if (child) {
          if (i > 0 && spacing != null && child.style) {
            const px = typeof spacing === 'number' ? spacing + 'px' : String(spacing);
            child.style[isRow ? 'marginLeft' : 'marginTop'] = px;
          }
          el.appendChild(child);
        }
      }
    }
    return el;
  }
  if (Array.isArray(v)) {
    const frag = document.createDocumentFragment();
    for (const c of v) {
      const child = vnodeToDom(c, invokeHandler, onAfterInvoke);
      if (child) frag.appendChild(child);
    }
    return frag;
  }
  return null;
}

/**
 * HookRenderer: run a hook and render into container.
 * Local: hookPath + baseUrl. Remote: host + hookPath (fetches from host).
 *
 * @param {HTMLElement} container - DOM element to render into
 * @param {object} options
 * @param {string} options.hookPath - Path (e.g. 'hooks/app.jsx')
 * @param {string} [options.host] - For remote: base URL to fetch from
 * @param {string} [options.baseUrl] - For local: base URL for resolving paths
 * @returns {Promise<void>}
 */
export async function renderHook(container, options = {}) {
  const { hookPath, host, baseUrl = getDefaultBaseUrl() } = options;
  let moduleMap = {};
  let source = '';
  let effectivePath = hookPath || 'hooks/app.jsx';
  let themeYaml = '';
  let hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };

  if (host) {
    const base = host.trim().replace(/\/+$/, '');
    const baseDir = `${base}/`;
    const cacheBust = '?t=' + Date.now();

    let hookHostSameOrigin = true;
    if (typeof location !== 'undefined' && location.origin) {
      try {
        hookHostSameOrigin = new URL(baseDir).origin === location.origin;
      } catch {
        hookHostSameOrigin = true;
      }
    }

    let entryPath = effectivePath;
    try {
      if (hookHostSameOrigin) {
        const optsResp = await fetch(baseDir, { method: 'OPTIONS' });
        if (optsResp.ok) {
          let p = optsResp.headers.get('x-hook-path') || optsResp.headers.get('x-relay-hook-path');
          if (!p && optsResp.headers.get('content-type')?.includes('application/json')) {
            const j = await optsResp.json();
            p = j?.hookPath ?? j?.hook_path ?? j?.path;
          }
          if (p) entryPath = p;
        }
      }
    } catch {
      /* discovery optional */
    }

    hooksManifest = { hooks: [] };
    if (hookHostSameOrigin) {
      const themeUrl = resolveHookUrl(baseDir, 'theme.yaml') + cacheBust;
      const hooksUrl = resolveHookUrl(baseDir, 'hooks.json') + cacheBust;
      const [themeResp, hooksResp] = await Promise.all([
        fetch(themeUrl).catch(() => null),
        fetch(hooksUrl).catch(() => null),
      ]);
      if (themeResp && themeResp.ok) {
        themeYaml = await themeResp.text();
      }
      if (hooksResp && hooksResp.ok) {
        try {
          hooksManifest = await hooksResp.json();
        } catch {
          hooksManifest = { hooks: [] };
        }
      }
    }

    moduleMap = {};
    const keyFromPath = (p) => {
      const s = (p || '').trim();
      if (s.startsWith('/')) return s.slice(1);
      return s.replace(/^\.\//, '');
    };
    const putModule = (pathKey, text) => {
      const k = (pathKey || '').trim();
      if (!k) return;
      moduleMap[k] = text;
      const n = keyFromPath(k);
      if (n && n !== k) moduleMap[n] = text;
    };
    for (const h of hooksManifest.hooks || []) {
      if (!h || !h.path) continue;
      const u = resolveHookUrl(baseDir, h.path) + cacheBust;
      const resp = await fetch(u);
      if (resp.ok) {
        putModule(h.path, await resp.text());
      } else {
        console.warn('[relay-runtime] remote hook fetch failed:', h.path, resp.status);
      }
    }

    effectivePath = keyFromPath(entryPath);

    if (!moduleMap[effectivePath]) {
      const srcUrl = resolveHookUrl(baseDir, entryPath) + cacheBust;
      const srcResp = await fetch(srcUrl);
      if (!srcResp.ok) throw new Error(`Fetch entry ${srcUrl} failed: ${srcResp.status}`);
      source = await srcResp.text();
      putModule(entryPath, source);
    } else {
      source = moduleMap[effectivePath];
    }

    if (!hooksManifest.hooks || hooksManifest.hooks.length === 0) {
      hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
    }
  } else {
    const url = resolveUrl(baseUrl, effectivePath);
    const r = await fetch(url);
    if (!r.ok) throw new Error(`Fetch ${url} failed: ${r.status}`);
    source = await r.text();
    moduleMap[effectivePath] = source;
    hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
  }

  const doRender = () => {
    try {
      const vnodeJson = run_hook_with_host(
        source,
        JSON.stringify(moduleMap),
        themeYaml,
        JSON.stringify(hooksManifest),
        effectivePath
      );
      const css = get_injected_css();
      const styleEl = document.getElementById(_styleId);
      if (styleEl) styleEl.textContent = (styleEl.textContent || '') + css;
      container.innerHTML = '';
      const vnode = JSON.parse(vnodeJson);
      const el = vnodeToDom(vnode, invoke_handler, doRender);
      if (el) container.appendChild(el);
    } catch (e) {
      container.textContent = `Render error: ${e?.message ?? e}`;
      container.className = (container.className || '') + ' error';
      throw e;
    }
  };
  doRender();
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
 * Resolve hook/theme/manifest path against a base URL.
 * Handles absolute URLs, root-relative paths (`/foo`), and relative paths (`./x`, `../x`).
 * Matches `new URL(path, baseWithSlash)` semantics.
 *
 * @param {string} baseUrl - Directory or origin base, e.g. `https://cdn.example.com/pkg/` or `http://10.0.2.2:8080`
 * @param {string} path - Path or URL to resolve
 * @returns {string}
 */
export function resolveHookUrl(baseUrl, path) {
  const p = (path ?? '').trim();
  if (!p) return baseUrl || '';
  if (/^https?:\/\//i.test(p)) return p;
  if (!baseUrl || baseUrl === '.') {
    try {
      const base =
        typeof document !== 'undefined' && document.baseURI
          ? document.baseURI
          : 'http://localhost/';
      return new URL(p, base).href;
    } catch {
      return p;
    }
  }
  const dir = baseUrl.endsWith('/') ? baseUrl : `${baseUrl}/`;
  try {
    return new URL(p, dir).href;
  } catch {
    return p;
  }
}

/** @param {string} baseUrl @param {string} path */
function resolveUrl(baseUrl, path) {
  return resolveHookUrl(baseUrl, path);
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

    // Forward window messages to relay-runtime so hooks can call getPendingMessages().
    // Skip messages with type "relay-runtime" (sent by postMessage) to avoid echo.
    if (typeof window !== 'undefined') {
      window.addEventListener('message', (e) => {
        try {
          const d = e.data;
          if (d && typeof d === 'object' && d.type === 'relay-runtime') return;
          const payload = typeof d === 'string' ? d : JSON.stringify(d ?? null);
          push_message_from_host(payload);
        } catch (_) { /* ignore */ }
      });
    }

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

    const doRender = () => {
      try {
        let vnodeJson;
        try {
          vnodeJson = run_hook_with_host(
            appSource,
            JSON.stringify(moduleMap),
            themeYaml,
            JSON.stringify(manifest),
            appPath || ''
          );
        } catch (e) {
          console.error('[relay-runtime] run_hook_with_host failed:', e);
          throw e;
        }
        const css = get_injected_css();
        const styleEl = document.getElementById(styleId);
        if (styleEl) styleEl.textContent = css;
        root.innerHTML = '';
        const vnode = JSON.parse(vnodeJson);
        const el = vnodeToDom(vnode, invoke_handler, doRender);
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
      } catch (e) {
        console.error('[relay-runtime] doRender failed:', e);
        throw e;
      }
    };
    doRender();

    // Event delegation: theme toggle (data-theme)
    root.addEventListener('click', (e) => {
      const theme = e.target?.dataset?.theme;
      if (theme) {
        setCurrentTheme(theme);
      }
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
