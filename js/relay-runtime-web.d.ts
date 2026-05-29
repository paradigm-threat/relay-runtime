/**
 * relay-runtime web integration
 */

export interface BootstrapOptions {
  rootId?: string;
  styleId?: string;
  themePath?: string;
  hooksPath?: string;
  appPath?: string;
  baseUrl?: string;
  wasmUrl?: string | URL;
}

/**
 * Bootstrap relay-runtime: load WASM, fetch config and hooks, run app, inject CSS, render to DOM.
 */
export function bootstrap(options?: BootstrapOptions): Promise<void>;

/**
 * Options for renderHook().
 */
export interface RenderHookOptions {
  hookPath?: string;
  host?: string;
  baseUrl?: string;
}

/**
 * Run a hook (local or remote) and render into container. Used by relay-hook-renderer.
 */
export function renderHook(
  container: HTMLElement,
  options?: RenderHookOptions
): Promise<void>;

/**
 * Resolve a hook/manifest path against a base URL (absolute, root-relative, relative).
 */
export function resolveHookUrl(baseUrl: string, path: string): string;

/**
 * Convert VNode JSON to DOM nodes.
 */
export function vnodeToDom(v: unknown): DocumentFragment | HTMLElement | Text | null;

/**
 * Get available theme names. Call after bootstrap().
 */
export function getThemes(): string[];

/**
 * Set current theme and update styles. Use with theme toggle buttons (data-theme="...").
 */
export function setCurrentTheme(name: string): void;
