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
