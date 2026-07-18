/**
 * Vendored from `relay-runtime/js/relay-runtime-web.js`.
 * Import path points at this package's wasm-pack output (see `npm run build:wasm`).
 */
import init, {
  run_hook_with_host,
  get_themes,
  set_theme_and_render,
  get_injected_css,
  push_message_from_host,
  invoke_handler,
  consume_render_requested,
  set_client_live,
  is_client_live,
  get_current_theme_name,
} from '../wasm/relay_runtime_pkg/relay_runtime.js';

let _styleId = 'relay-ssr-theme';

/** Ensure a <style id=...> exists in <head>; prefer SSR-injected #relay-ssr-theme when present. */
function ensureStyleElement(id) {
  if (typeof document === 'undefined') return null;
  let el = document.getElementById(id);
  if (!el) {
    el = document.createElement('style');
    el.id = id;
    document.head.appendChild(el);
  }
  return el;
}

/** Apply CSS from the last run_hook_with_host into the document stylesheet. */
function applyInjectedCss() {
  try {
    const css = typeof get_injected_css === 'function' ? get_injected_css() : '';
    if (css && String(css).trim()) {
      const styleEl = ensureStyleElement(_styleId);
      if (styleEl) styleEl.textContent = String(css);
    }
  } catch (_) {
    /* optional */
  }
  syncHtmlThemeClass();
}

/** Mirror CleverScript/WASM theme name onto <html class="light|dark"> for system dual CSS. */
function syncHtmlThemeClass() {
  if (typeof document === 'undefined') return;
  let name = '';
  try {
    if (typeof get_current_theme_name === 'function') {
      name = String(get_current_theme_name() || '').trim();
    }
  } catch (_) {
    return;
  }
  if (name !== 'light' && name !== 'dark') return;
  const root = document.documentElement;
  root.classList.remove('light', 'dark');
  root.classList.add(name);
}

const unitlessStyleProperties = new Set([
  'animationIterationCount', 'columnCount', 'fillOpacity', 'flex', 'flexGrow',
  'flexShrink', 'fontWeight', 'gridColumn', 'gridRow', 'lineHeight', 'opacity',
  'order', 'orphans', 'scale', 'strokeOpacity', 'strokeWidth', 'tabSize',
  'widows', 'zIndex', 'zoom',
]);

/** Match React style semantics: non-zero numeric dimensions become CSS pixels. */
function normalizeStyleValue(property, value) {
  const stringValue = String(value).trim();
  if (
    stringValue === ''
    || property.startsWith('--')
    || unitlessStyleProperties.has(property)
    || !/^[+-]?(?:\d+\.?\d*|\.\d+)(?:e[+-]?\d+)?$/i.test(stringValue)
    || Number(stringValue) === 0
  ) {
    return stringValue;
  }
  return `${stringValue}px`;
}

/**
 * Install globalThis.__relay_host_fetch(url, initJson) for CleverScript Host.fetch (sync XHR).
 * Safe to call multiple times; first wins unless already set.
 */
export function installRelayHostFetch() {
  if (typeof globalThis === 'undefined') return;
  if (typeof globalThis.__relay_host_fetch === 'function') return;
  globalThis.__relay_host_fetch = (url, initJson) => {
    let method = 'GET';
    let headers = null;
    try {
      if (initJson && String(initJson).length) {
        const o = JSON.parse(String(initJson));
        if (o && o.method) method = String(o.method).toUpperCase();
        if (o && o.headers && typeof o.headers === 'object') headers = o.headers;
      }
    } catch {
      /* ignore */
    }
    let fetchUrl = String(url ?? '');
    if (fetchUrl && !/^https?:\/\//i.test(fetchUrl)) {
      const base = globalThis.__relay_fetch_base_url;
      if (typeof base === 'string' && base.trim()) {
        try {
          const baseNorm = base.trim().replace(/\/+$/, '') + '/';
          fetchUrl = new URL(fetchUrl, baseNorm).href;
        } catch {
          /* keep original url */
        }
      }
    }
    try {
      const xhr = new XMLHttpRequest();
      xhr.open(method, fetchUrl, false);
      const branch = globalThis.__relay_fetch_branch;
      if (typeof branch === 'string' && branch.trim()) {
        xhr.setRequestHeader('X-Relay-Branch', branch.trim());
      }
      if (headers) {
        for (const [k, v] of Object.entries(headers)) {
          if (v == null) continue;
          xhr.setRequestHeader(String(k), String(v));
        }
      }
      xhr.send(null);
      const ok = xhr.status >= 200 && xhr.status < 300;
      return JSON.stringify({
        ok,
        status: xhr.status,
        text: xhr.responseText != null ? String(xhr.responseText) : '',
      });
    } catch (e) {
      return JSON.stringify({
        ok: false,
        status: 0,
        text: '',
        error: String(e?.message ?? e),
      });
    }
  };
}

/** Resolve CleverScript `fetch('/path')` against remote hook gateway base. */
export function setRelayFetchBaseUrl(baseUrl) {
  if (typeof globalThis === 'undefined') return;
  const b = (baseUrl ?? '').trim().replace(/\/+$/, '');
  globalThis.__relay_fetch_base_url = b || undefined;
}

/** Prefer the same branch SSR used (`X-Relay-Branch` on host fetches). */
export function setRelayFetchBranch(branch) {
  if (typeof globalThis === 'undefined') return;
  const b = (branch ?? '').trim();
  globalThis.__relay_fetch_branch = b || undefined;
}

async function fetchThemeYamlForHost(baseDir, branch) {
  const paths = [
    'hooks/client/theme.yaml',
    'theme.yaml',
    'hooks/theme.yaml',
  ];
  const headers = {};
  if (branch) headers['X-Relay-Branch'] = branch;
  for (const p of paths) {
    try {
      const resp = await fetch(resolveHookUrl(baseDir, p) + '?t=' + Date.now(), { headers });
      if (resp.ok) {
        const text = await resp.text();
        if (text && text.trim()) return text;
      }
    } catch {
      /* try next */
    }
  }
  return '';
}

/** Resolve relative media paths against hook gateway base. */
function resolveMediaUrl(url) {
  const u = String(url ?? '').trim();
  if (!u) return u;
  if (/^https?:\/\//i.test(u) || /^blob:/i.test(u) || /^data:/i.test(u)) return u;
  const base = globalThis.__relay_fetch_base_url;
  if (typeof base === 'string' && base.trim()) {
    try {
      return new URL(u, base.trim().replace(/\/+$/, '') + '/').href;
    } catch {
      return u;
    }
  }
  return u;
}

function propStringFromVnodeProps(props, key) {
  const val = props && props[key];
  if (val && typeof val === 'object' && val.type === 'String') return val.value;
  if (typeof val === 'string') return val;
  return null;
}

function propBoolFromVnodePropsLoose(props, key) {
  const val = props && props[key];
  if (val && typeof val === 'object' && val.type === 'Bool') return !!val.value;
  if (typeof val === 'boolean') return val;
  return null;
}

function propNumberFromVnodeProps(props, key) {
  const val = props && props[key];
  if (val && typeof val === 'object' && val.type === 'Number') return val.value;
  if (typeof val === 'number') return val;
  return null;
}

/** Minimal SRT → WebVTT for `<track src>` (browser requires WebVTT). */
function srtToVtt(srt) {
  let body = String(srt).replace(/\r/g, '').trim();
  if (body.startsWith('WEBVTT')) return body;
  body = body.replace(/(\d{2}:\d{2}:\d{2}),(\d{3})/g, '$1.$2');
  return 'WEBVTT\n\n' + body;
}

function fetchTextSync(url) {
  if (typeof globalThis.__relay_host_fetch !== 'function') return null;
  try {
    const raw = globalThis.__relay_host_fetch(url, '{}');
    const j = JSON.parse(raw);
    if (j && j.ok && typeof j.text === 'string') return j.text;
  } catch {
    /* ignore */
  }
  return null;
}

function attachMediaHandler(el, eventName, props, key, invokeHandler, onAfterInvoke, payloadFn) {
  const val = props && props[key];
  if (!val || typeof val !== 'object' || val.type !== 'HandlerId' || typeof val.value !== 'number') return;
  const handlerId = val.value;
  el.addEventListener(eventName, () => {
    try {
      if (payloadFn) {
        const payload = payloadFn();
        if (payload !== undefined) {
          push_message_from_host(
            JSON.stringify({ type: 'relay-video', event: key, handlerId, ...payload }),
          );
        }
      }
      invokeHandler(handlerId);
    } catch (err) {
      console.error('[relay-runtime] video handler failed:', err);
    }
    if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
  });
}

/**
 * Build native HTML5 video shell for `relay-video-player` VNodes.
 * CleverScript controls: playing, currentTime, command/commandSeq/commandArg props.
 */
function buildRelayVideoPlayer(props, invokeHandler, onAfterInvoke) {
  const shell = document.createElement('div');
  shell.className = 'relay-video-player-shell';
  const video = document.createElement('video');
  video.className = 'relay-video-player';
  video.setAttribute('playsinline', '');
  video.setAttribute('preload', 'metadata');

  const src = resolveMediaUrl(propStringFromVnodeProps(props, 'src'));
  if (src) video.src = src;
  const poster = propStringFromVnodeProps(props, 'poster');
  if (poster) video.poster = resolveMediaUrl(poster);

  const controls = propBoolFromVnodePropsLoose(props, 'controls');
  video.controls = controls !== false;
  const autoPlay = propBoolFromVnodePropsLoose(props, 'autoPlay');
  if (autoPlay === true) video.autoplay = true;
  const muted = propBoolFromVnodePropsLoose(props, 'muted');
  if (muted === true) video.muted = true;
  const loop = propBoolFromVnodePropsLoose(props, 'loop');
  if (loop === true) video.loop = true;

  const vol = propNumberFromVnodeProps(props, 'volume');
  if (vol != null && !Number.isNaN(vol)) video.volume = Math.min(1, Math.max(0, vol));
  const rate = propNumberFromVnodeProps(props, 'playbackRate');
  if (rate != null && !Number.isNaN(rate)) video.playbackRate = rate;

  const playerId = propStringFromVnodeProps(props, 'playerId') || `rvp-${Math.random().toString(36).slice(2)}`;
  video.dataset.relayPlayerId = playerId;

  const subtitlesJson = propStringFromVnodeProps(props, 'subtitlesJson');
  if (subtitlesJson) {
    try {
      const tracks = JSON.parse(subtitlesJson);
      if (Array.isArray(tracks)) {
        tracks.forEach((t, i) => {
          if (!t || !t.src) return;
          const track = document.createElement('track');
          track.kind = 'subtitles';
          const trackUrl = resolveMediaUrl(String(t.src));
          let srcForTrack = trackUrl;
          if (/\.srt$/i.test(trackUrl)) {
            const srt = fetchTextSync(trackUrl);
            if (srt) {
              srcForTrack = URL.createObjectURL(new Blob([srtToVtt(srt)], { type: 'text/vtt' }));
            }
          }
          track.src = srcForTrack;
          if (t.label) track.label = String(t.label);
          if (t.lang) track.srclang = String(t.lang);
          if (t.default) track.default = true;
          video.appendChild(track);
        });
      }
    } catch (e) {
      console.warn('[relay-runtime] subtitlesJson parse failed', e);
    }
  }

  attachMediaHandler(video, 'play', props, 'onPlay', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'pause', props, 'onPause', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'ended', props, 'onEnded', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'error', props, 'onError', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'loadedmetadata', props, 'onLoadedMetadata', invokeHandler, onAfterInvoke);
  attachMediaHandler(video, 'seeked', props, 'onSeeked', invokeHandler, onAfterInvoke);

  let lastTimeEvent = 0;
  video.addEventListener('timeupdate', () => {
    const now = Date.now();
    if (now - lastTimeEvent < 250) return;
    lastTimeEvent = now;
    const val = props && props.onTimeUpdate;
    if (!val || val.type !== 'HandlerId') return;
    try {
      push_message_from_host(
        JSON.stringify({
          type: 'relay-video',
          event: 'onTimeUpdate',
          handlerId: val.value,
          playerId,
          currentTime: video.currentTime,
          duration: video.duration,
        }),
      );
      invokeHandler(val.value);
    } catch (err) {
      console.error('[relay-runtime] onTimeUpdate failed', err);
    }
    if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
  });

  const playing = propBoolFromVnodePropsLoose(props, 'playing');
  if (playing === true) {
    video.play().catch(() => {});
  } else if (playing === false) {
    video.pause();
  }

  const ct = propNumberFromVnodeProps(props, 'currentTime');
  if (ct != null && !Number.isNaN(ct) && Math.abs(video.currentTime - ct) > 0.25) {
    video.currentTime = ct;
  }

  const cmdSeq = propNumberFromVnodeProps(props, 'commandSeq');
  const cmd = propNumberFromVnodeProps(props, 'command');
  const cmdArg = propNumberFromVnodeProps(props, 'commandArg');
  if (cmdSeq != null && String(cmdSeq) !== video.dataset.relayCommandSeq) {
    video.dataset.relayCommandSeq = String(cmdSeq);
    if (cmd === 1) video.play().catch(() => {});
    else if (cmd === 2) video.pause();
    else if (cmd === 3 && cmdArg != null) video.currentTime = cmdArg;
  }

  shell.appendChild(video);
  return shell;
}

/** VNode PropValue.Bool from relay-runtime serde: `{ type: 'Bool', value: true }`. */
export function propBoolFromVnodeProps(props, key) {
  const val = props && props[key];
  return !!(val && typeof val === 'object' && val.type === 'Bool' && val.value === true);
}

/**
 * When relayControlled anchor: only intercept ordinary left-clicks (SPA); let modified / special targets behave natively.
 */
export function shouldInterceptRelayControlledAnchorClick(e, el) {
  if (e.defaultPrevented || e.button !== 0) return false;
  if (e.metaKey || e.ctrlKey || e.shiftKey || e.altKey) return false;
  const t = el.getAttribute('target');
  if (t && t !== '_self') return false;
  const href = el.getAttribute('href');
  if (!href) return true;
  if (/^(mailto:|tel:|#)/i.test(href)) return false;
  return true;
}

/**
 * Attach event handlers from VNode props onto an existing DOM element (hydration).
 */
function attachHandlersFromProps(el, tag, props, invokeHandler, onAfterInvoke) {
  if (!props || !el || typeof el.addEventListener !== 'function') return;
  const relayControlled = propBoolFromVnodeProps(props, 'relayControlled');
  for (const [k, val] of Object.entries(props)) {
    if (
      val &&
      typeof val === 'object' &&
      (val.type === 'Handler' || val.type === 'HandlerId') &&
      typeof val.value === 'number'
    ) {
      const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
      if (tag === 'a' && eventType === 'click' && !relayControlled) {
        continue;
      }
      const handlerId = val.value;
      el.addEventListener(eventType, (e) => {
        if (tag === 'a' && eventType === 'click' && relayControlled) {
          if (!shouldInterceptRelayControlledAnchorClick(e, el)) return;
          e.preventDefault();
        }
        try {
          invokeHandler(handlerId);
        } catch (err) {
          console.error('[relay-runtime] Handler invocation failed:', err);
          throw err;
        }
        if (onAfterInvoke && consume_render_requested()) onAfterInvoke();
      });
    }
  }
}

function elementChildNodes(parent) {
  const out = [];
  if (!parent || !parent.childNodes) return out;
  for (const n of parent.childNodes) {
    if (n.nodeType === 1) out.push(n); // Element
  }
  return out;
}

function expectedDomTag(tag) {
  if (tag === 'relay-video-player' || tag === 'relay-hook-renderer') return 'DIV';
  if (tag === 'fragment') return null;
  return String(tag || 'div').toUpperCase();
}

/**
 * Hydrate: walk server HTML + VNode in lockstep and attach handlers. Returns false on mismatch.
 *
 * @param {object} v - VNode JSON
 * @param {HTMLElement} container - Element whose children are the SSR markup (e.g. #root)
 * @param {function(number): void} [invokeHandler]
 * @param {function(): void} [onAfterInvoke]
 * @returns {boolean}
 */
export function hydrateVnode(v, container, invokeHandler = invoke_handler, onAfterInvoke = null) {
  if (!v || !container) return false;
  try {
    if (v.Empty !== undefined) return true;
    if (v.Text !== undefined) return true;
    if (v.Element) {
      const { tag, children } = v.Element;
      if (tag === 'fragment') {
        return hydrateChildren(children || [], elementChildNodes(container), invokeHandler, onAfterInvoke);
      }
      const kids = elementChildNodes(container);
      if (kids.length === 1) {
        return hydrateElementNode(v, kids[0], invokeHandler, onAfterInvoke);
      }
      // Single text-only or empty
      if (kids.length === 0 && (!children || children.length === 0)) return true;
      return false;
    }
    if (Array.isArray(v)) {
      return hydrateChildren(v, elementChildNodes(container), invokeHandler, onAfterInvoke);
    }
    return false;
  } catch (e) {
    console.warn('[relay-runtime] hydrateVnode failed:', e);
    return false;
  }
}

function hydrateChildren(vnodes, domEls, invokeHandler, onAfterInvoke) {
  const elements = (vnodes || []).filter((c) => c && c.Element && c.Element.tag !== 'fragment');
  // Expand fragments
  const flat = [];
  for (const c of vnodes || []) {
    if (!c || c.Empty !== undefined || c.Text !== undefined) continue;
    if (c.Element && c.Element.tag === 'fragment') {
      for (const nested of c.Element.children || []) {
        if (nested && nested.Element) flat.push(nested);
      }
    } else if (c.Element) {
      flat.push(c);
    }
  }
  if (flat.length !== domEls.length) {
    // Allow soft mismatch when only text nodes differed in SSR
    if (flat.length === 0 && domEls.length === 0) return true;
    return false;
  }
  for (let i = 0; i < flat.length; i++) {
    if (!hydrateElementNode(flat[i], domEls[i], invokeHandler, onAfterInvoke)) return false;
  }
  return true;
}

function hydrateElementNode(v, el, invokeHandler, onAfterInvoke) {
  if (!v || !v.Element || !el) return false;
  const { tag, props, children } = v.Element;
  if (tag === 'fragment') {
    return hydrateChildren(children || [], elementChildNodes(el), invokeHandler, onAfterInvoke);
  }
  const expect = expectedDomTag(tag);
  if (expect && el.tagName !== expect) {
    // Soft: video/hook stubs use div
    if (!(expect === 'DIV' && el.tagName === 'DIV')) {
      return false;
    }
  }
  attachHandlersFromProps(el, tag, props || {}, invokeHandler, onAfterInvoke);
  return hydrateChildren(children || [], elementChildNodes(el), invokeHandler, onAfterInvoke);
}

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
        renderHookErrorUI(container, e, {
          keepContent: container.dataset?.relaySsr === '1',
        });
      });
      return container;
    }
    if (tag === 'relay-video-player') {
      return buildRelayVideoPlayer(props || {}, invokeHandler, onAfterInvoke);
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
      const relayControlled = propBoolFromVnodeProps(props, 'relayControlled');
      for (const [k, val] of Object.entries(props)) {
        if (val && typeof val === 'object' && (val.type === 'Handler' || val.type === 'HandlerId') && typeof val.value === 'number') {
          const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
          if (tag === 'a' && eventType === 'click' && !relayControlled) {
            continue;
          }
          const handlerId = val.value;
          el.addEventListener(eventType, (e) => {
            if (tag === 'a' && eventType === 'click' && relayControlled) {
              if (!shouldInterceptRelayControlledAnchorClick(e, el)) return;
              e.preventDefault();
            }
            try {
              invokeHandler(handlerId);
            } catch (err) {
              console.error('[relay-runtime] Handler invocation failed:', err);
              throw err;
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
                el.style[pk] = normalizeStyleValue(pk, pv);
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
    const rawTag = v.tag === 'fragment' ? 'div' : v.tag;
    if (rawTag === 'relay-video-player') {
      return buildRelayVideoPlayer(v.props || {}, invokeHandler, onAfterInvoke);
    }
    const el = document.createElement(rawTag);
    if (v.props) {
      const relayControlled = propBoolFromVnodeProps(v.props, 'relayControlled');
      for (const [k, val] of Object.entries(v.props)) {
        if (val && typeof val === 'object' && (val.type === 'Handler' || val.type === 'HandlerId') && typeof val.value === 'number') {
          const eventType = k.startsWith('on') ? k.slice(2).toLowerCase() : k;
          if (rawTag === 'a' && eventType === 'click' && !relayControlled) {
            continue;
          }
          const handlerId = val.value;
          el.addEventListener(eventType, (e) => {
            if (rawTag === 'a' && eventType === 'click' && relayControlled) {
              if (!shouldInterceptRelayControlledAnchorClick(e, el)) return;
              e.preventDefault();
            }
            try {
              invokeHandler(handlerId);
            } catch (err) {
              console.error('[relay-runtime] Handler invocation failed:', err);
              throw err;
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
                el.style[pk] = normalizeStyleValue(pk, pv);
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
 * Build a readable Error from a failed hook fetch. The gateway 500 body
 * (get_hook_execution_error_response) carries `summaryForReaders`, `technicalReport`,
 * repo/branch, and structured `cleverScript` diagnostics — surface them instead of
 * just "failed: 500". The parsed body is kept on `err.gateway` for advanced UI.
 */
async function hookFetchError(resp, url) {
  const err = new Error(`Fetch entry ${url} failed: ${resp.status}`);
  err.status = resp.status;
  err.url = url;
  let text = '';
  try {
    text = await resp.text();
  } catch {
    return err;
  }
  let detail = null;
  try {
    detail = JSON.parse(text);
  } catch {
    /* not JSON */
  }
  if (!detail || typeof detail !== 'object') {
    if (text.trim()) err.message += `\n${text.trim().slice(0, 500)}`;
    return err;
  }
  err.gateway = detail;
  const parts = [err.message];
  if (detail.repo || detail.branch) {
    parts.push(`repo: ${detail.repo ?? '?'}  branch: ${detail.branch ?? '?'}`);
  }
  const cs = detail.cleverScript;
  if (cs && typeof cs === 'object' && cs.message) {
    const loc = [cs.file, cs.line, cs.column].filter((v) => v != null && v !== '').join(':');
    parts.push(`CleverScript ${cs.type || 'error'}${loc ? ` at ${loc}` : ''}: ${cs.message}`);
    if (cs.source_line) {
      parts.push(`  ${cs.source_line}`);
      if (cs.column) parts.push(`  ${' '.repeat(Math.max(0, cs.column - 1))}^`);
    }
    if (Array.isArray(cs.call_stack) && cs.call_stack.length) {
      parts.push(`  call stack: ${cs.call_stack.join(' > ')}`);
    }
  } else if (typeof detail.technicalReport === 'string' && detail.technicalReport) {
    parts.push(detail.technicalReport.slice(0, 1000));
  }
  if (typeof detail.summaryForReaders === 'string' && detail.summaryForReaders) {
    parts.push(detail.summaryForReaders);
  }
  err.message = parts.join('\n');
  return err;
}

/**
 * Render a structured hook error panel into (or on top of) `container`.
 * When `keepContent` is true (e.g. SSR markup is still valid), the panel is prepended
 * so readers keep the static page; otherwise the container is replaced.
 * Shows: title, message (pre-wrap), fetch URL + HTTP status, repo/branch, and
 * CleverScript diagnostics (type, file:line:column, source line, call stack) when the
 * gateway 500 JSON was attached to the error (see hookFetchError).
 */
/**
 * WASM parse/runtime errors arrive as JSON strings `{message, line, column, file?, source_line?}`
 * (see run_hook_with_file docs). Format them as `file:line:column: message` with a caret line.
 */
function formatRuntimeErrorMessage(msg) {
  const s = String(msg ?? '');
  const start = s.indexOf('{');
  if (start === -1) return s;
  try {
    const parsed = JSON.parse(s.slice(start));
    if (!parsed || typeof parsed.message !== 'string') return s;
    const loc = [parsed.file, parsed.line, parsed.column].filter((v) => v != null && v !== '').join(':');
    let out = (s.slice(0, start) + (loc ? `${loc}: ` : '') + parsed.message).trim();
    if (parsed.source_line) {
      out += `\n  ${parsed.source_line}\n  ${' '.repeat(Math.max(0, (parsed.column || 1) - 1))}^`;
    }
    return out;
  } catch {
    return s;
  }
}

export function renderHookErrorUI(container, err, { keepContent = false, title = 'Hook error' } = {}) {
  if (!container) return;
  const panel = document.createElement('div');
  panel.className = 'relay-hook-error';
  panel.style.cssText =
    'font: 13px/1.5 ui-monospace, monospace; color: #ffb4b4; background: #2a0f0f;' +
    'border: 1px solid #7f1d1d; border-radius: 6px; padding: 10px 12px; margin: 8px;' +
    'white-space: pre-wrap; overflow-wrap: anywhere; text-align: left;';

  const head = document.createElement('div');
  head.style.cssText = 'font-weight: bold; color: #fecaca; margin-bottom: 4px;';
  head.textContent = title;
  panel.appendChild(head);

  const msg = document.createElement('div');
  msg.textContent = formatRuntimeErrorMessage(err?.message ?? err ?? 'Unknown error');
  panel.appendChild(msg);

  const meta = [];
  if (err?.status) meta.push(`HTTP ${err.status}`);
  const gw = err?.gateway;
  if (gw?.repo) meta.push(`repo ${gw.repo}`);
  if (gw?.branch) meta.push(`branch ${gw.branch}`);
  if (typeof location !== 'undefined') meta.push(`page ${location.pathname}${location.search}`);
  meta.push(`at ${new Date().toISOString()}`);
  if (meta.length) {
    const info = document.createElement('div');
    info.style.cssText = 'margin-top: 6px; color: #f87171; opacity: 0.8; font-size: 12px;';
    info.textContent = meta.join(' · ');
    panel.appendChild(info);
  }

  const hint = document.createElement('div');
  hint.style.cssText = 'margin-top: 6px; color: #fca5a5; opacity: 0.8; font-size: 12px;';
  hint.textContent = keepContent
    ? 'Static server-rendered content is shown below; interactive features are unavailable.'
    : 'Details are also logged to the browser console.';
  panel.appendChild(hint);

  if (keepContent) {
    // Errors can surface twice (renderHook catch + caller catch) — keep one panel.
    const existing = container.querySelector(':scope > .relay-hook-error');
    if (existing) existing.replaceWith(panel);
    else container.prepend(panel);
  } else {
    container.innerHTML = '';
    container.className = (container.className || '') + ' error';
    container.appendChild(panel);
  }
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
  const {
    hookPath,
    host,
    baseUrl = getDefaultBaseUrl(),
    hydrate = false,
    branch = '',
    initialPath = '',
    themeYaml = '',
  } = options;
  let moduleMap = {};
  let source = '';
  let effectivePath = hookPath || 'hooks/app.jsx';
  let hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
  let hydratedOnce = false;
  let livePromoted = false;
  let themeYamlText = typeof themeYaml === 'string' ? themeYaml : '';
  const branchNorm = typeof branch === 'string' ? branch.trim() : '';
  const initialPathNorm = typeof initialPath === 'string' ? initialPath.trim() : '';

  try {
    if (typeof set_client_live === 'function') set_client_live(false);
  } catch (_) {
    /* optional until wasm rebuild */
  }

  if (host) {
    const base = host.trim().replace(/\/+$/, '');
    const baseDir = `${base}/`;
    const cacheBust = '?t=' + Date.now();
    const branchHeaders = branchNorm ? { 'X-Relay-Branch': branchNorm } : {};

    let hookHostSameOrigin = true;
    if (typeof location !== 'undefined' && location.origin) {
      try {
        hookHostSameOrigin = new URL(baseDir).origin === location.origin;
      } catch {
        hookHostSameOrigin = true;
      }
    }

    // Prefer explicit hookPath (e.g. #relay-embed-config) over OPTIONS discovery so
    // SSR hydrate and CSR shell can target different entry hooks on the same origin.
    const explicitHookPath = typeof hookPath === 'string' && hookPath.trim().length > 0;
    let entryPath = effectivePath;
    try {
      // Cross-origin OPTIONS preflights; static mirrors often lack
      // Access-Control-Allow-Methods: OPTIONS. Skip when origins differ.
      if (!explicitHookPath && hookHostSameOrigin) {
        const optsResp = await fetch(baseDir, { method: 'OPTIONS', headers: branchHeaders });
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
    // Same-origin: safe to probe OPTIONS above. Cross-origin: still GET theme + hooks manifest
    // when the static host sends Access-Control-Allow-Origin (e.g. `serve --cors`).
    const hooksUrl = resolveHookUrl(baseDir, 'hooks.json') + cacheBust;
    const hooksResp = await fetch(hooksUrl, { headers: branchHeaders }).catch(() => null);
    if (hooksResp && hooksResp.ok) {
      try {
        hooksManifest = await hooksResp.json();
      } catch {
        hooksManifest = { hooks: [] };
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
      const resp = await fetch(u, { headers: branchHeaders });
      if (resp.ok) {
        putModule(h.path, await resp.text());
      } else {
        console.warn('[relay-runtime] remote hook fetch failed:', h.path, resp.status);
      }
    }

    effectivePath = keyFromPath(entryPath);

    if (!moduleMap[effectivePath]) {
      const srcUrl = resolveHookUrl(baseDir, entryPath) + cacheBust;
      const srcResp = await fetch(srcUrl, { headers: branchHeaders });
      if (!srcResp.ok) throw await hookFetchError(srcResp, srcUrl);
      source = await srcResp.text();
      putModule(entryPath, source);
    } else {
      source = moduleMap[effectivePath];
    }

    if (!hooksManifest.hooks || hooksManifest.hooks.length === 0) {
      hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
    }
    const baseNorm = base.replace(/\/+$/, '');
    hooksManifest.relayClientBaseUrl = baseNorm;
    // CleverScript: avoid building URLs with '/' in hook source; optional full URL for repo README.
    hooksManifest.relayDefaultMarkdownUrl = `${baseNorm}/README.md`;
    hooksManifest.relayPathSlash = '/';
    if (initialPathNorm) {
      hooksManifest.relayInitialPath = initialPathNorm.startsWith('/')
        ? initialPathNorm
        : `/${initialPathNorm}`;
    }
    if (branchNorm) {
      hooksManifest.relayBranch = branchNorm;
    }
    setRelayFetchBaseUrl(baseNorm);
    setRelayFetchBranch(branchNorm);
    if (!themeYamlText) {
      themeYamlText = await fetchThemeYamlForHost(baseDir, branchNorm);
    }
    if (container && container.dataset) {
      container.dataset.relayClientBaseUrl = baseNorm;
      if (branchNorm) container.dataset.relayBranch = branchNorm;
      if (initialPathNorm) {
        container.dataset.relayInitialPath = hooksManifest.relayInitialPath || initialPathNorm;
      }
    }
  } else {
    const url = resolveUrl(baseUrl, effectivePath);
    const r = await fetch(url);
    if (!r.ok) throw await hookFetchError(r, url);
    source = await r.text();
    moduleMap[effectivePath] = source;
    hooksManifest = { hooks: [{ path: effectivePath, name: 'Default' }] };
    if (initialPathNorm) {
      hooksManifest.relayInitialPath = initialPathNorm.startsWith('/')
        ? initialPathNorm
        : `/${initialPathNorm}`;
    }
    if (branchNorm) {
      hooksManifest.relayBranch = branchNorm;
      setRelayFetchBranch(branchNorm);
    }
  }

  const doRender = () => {
    try {
      let vnodeJson;
      for (let pass = 0; pass < 16; pass++) {
        vnodeJson = run_hook_with_host(
          source,
          JSON.stringify(moduleMap),
          themeYamlText || '',
          JSON.stringify(hooksManifest),
          effectivePath
        );
        if (!consume_render_requested()) break;
      }
      applyInjectedCss();
      let vnode = JSON.parse(vnodeJson);
      const tryHydrate = hydrate && !hydratedOnce && container.dataset?.relaySsr === '1';
      if (tryHydrate) {
        const ok = hydrateVnode(vnode, container, invoke_handler, doRender);
        if (ok) {
          hydratedOnce = true;
          container.dataset.relayHydrated = '1';
          // Promote to live and re-render so isClientLive() enables controls.
          try {
            if (typeof set_client_live === 'function') set_client_live(true);
          } catch (_) { /* optional */ }
          livePromoted = true;
          doRender();
          return;
        }
        console.warn('[relay-runtime] SSR hydrate mismatch; falling back to client render');
      }
      container.innerHTML = '';
      delete container.dataset.relaySsr;
      if (!livePromoted) {
        try {
          if (typeof set_client_live === 'function') set_client_live(true);
        } catch (_) { /* optional */ }
        livePromoted = true;
        // Re-run with isClientLive() true so interactive controls enable.
        for (let pass = 0; pass < 16; pass++) {
          vnodeJson = run_hook_with_host(
            source,
            JSON.stringify(moduleMap),
            themeYamlText || '',
            JSON.stringify(hooksManifest),
            effectivePath
          );
          if (!consume_render_requested()) break;
        }
        applyInjectedCss();
        vnode = JSON.parse(vnodeJson);
      }
      const el = vnodeToDom(vnode, invoke_handler, doRender);
      if (el) container.appendChild(el);
    } catch (e) {
      // SSR markup (if any) is still intact when render fails before the innerHTML wipe;
      // keep it readable and show diagnostics on top instead of blanking the page.
      renderHookErrorUI(container, e, {
        title: 'Render error',
        keepContent: container.dataset?.relaySsr === '1',
      });
      throw e;
    }
  };

  installEmbedActionDelegation(container, () => {
    doRender();
  });
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
 * Bootstrap relay-runtime: load WASM, fetch hooks manifest, run app, render to DOM.
 * Theme YAML is loaded by hook `<StyleContext src="…">` (`@clevertree/style`).
 *
 * @param {object} [options]
 * @param {string} [options.rootId='root'] - DOM id for mounting
 * @param {string} [options.hooksPath='hooks.json']
 * @param {string} [options.appPath='hooks/app.jsx']
 * @param {string} [options.baseUrl='.'] - Base URL for resolving paths (relative to page)
 * @param {string|URL} [options.wasmUrl] - Override WASM module URL (default: next to relay_runtime.js)
 * @returns {Promise<void>}
 */
export async function bootstrap(options = {}) {
  const opts = { ...DEFAULTS, ...options };
  const { rootId, hooksPath, appPath, baseUrl, wasmUrl } = opts;

  const root = document.getElementById(rootId);
  if (!root) {
    console.error('[relay-runtime] Root element not found:', rootId);
    return;
  }

  try {
    await init(wasmUrl);
    installRelayHostFetch();

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

    const hooksUrl = resolveUrl(baseUrl, hooksPath);
    const appUrl = resolveUrl(baseUrl, appPath);
    const cacheBust = '?t=' + Date.now();

    const [hooksResp, appResp] = await Promise.all([
      fetch(hooksUrl + cacheBust),
      fetch(appUrl + cacheBust),
    ]);

    if (!hooksResp.ok) {
      console.warn('[relay-runtime] hooks fetch failed:', hooksResp.status, hooksUrl);
    }
    if (!appResp.ok) {
      console.warn('[relay-runtime] app fetch failed:', appResp.status, appUrl);
    }

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
          for (let pass = 0; pass < 16; pass++) {
            vnodeJson = run_hook_with_host(
              appSource,
              JSON.stringify(moduleMap),
              '',
              JSON.stringify(manifest),
              appPath || ''
            );
            if (!consume_render_requested()) break;
          }
        } catch (e) {
          console.error('[relay-runtime] run_hook_with_host failed:', e);
          throw e;
        }
        applyInjectedCss();
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
 * Set current theme and update styles. Use with theme toggle buttons (`data-theme="…"`)
 * or CleverScript `setCurrentTheme` from `@clevertree/style-internal`.
 * @param {string} name - Theme name (e.g. 'light', 'dark')
 */
export function setCurrentTheme(name) {
  const n = String(name || '').trim();
  if (typeof document !== 'undefined') {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    if (n === 'light' || n === 'dark') {
      root.classList.add(n);
    }
  }
  let css = '';
  try {
    css = typeof set_theme_and_render === 'function' ? set_theme_and_render(n) : '';
  } catch (_) {
    /* optional */
  }
  if (!css || !String(css).trim()) {
    try {
      css = typeof get_injected_css === 'function' ? get_injected_css() : '';
    } catch (_) {
      css = '';
    }
  }
  if (css && String(css).trim()) {
    const styleEl = ensureStyleElement(_styleId);
    if (styleEl) styleEl.textContent = String(css);
  }
}

/** Install click delegation for `data-theme` and `data-relay-action` on a mount root. */
function installEmbedActionDelegation(root, onThemeChange) {
  if (!root || root.dataset.relayActionsBound === '1') return;
  root.dataset.relayActionsBound = '1';
  root.addEventListener('click', (e) => {
    const actionEl = e.target?.closest?.('[data-relay-action]');
    if (actionEl && !actionEl.disabled) {
      const action = actionEl.getAttribute('data-relay-action');
      if (action === 'print' && typeof window !== 'undefined' && typeof window.print === 'function') {
        e.preventDefault();
        window.print();
        return;
      }
    }
    const themeEl = e.target?.closest?.('[data-theme]');
    if (themeEl && !themeEl.disabled) {
      const theme = themeEl.getAttribute('data-theme');
      if (theme) {
        setCurrentTheme(theme);
        if (typeof onThemeChange === 'function') onThemeChange(theme);
      }
    }
  });
}
