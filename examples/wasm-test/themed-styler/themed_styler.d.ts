/* tslint:disable */
/* eslint-disable */

export function get_android_styles(state_json: string, selector: string, classes_json: string): string;

/**
 * Return the embedded default state as a JSON string.
 */
export function get_default_state_json(): string;

/**
 * Get all theme keys and names as JSON array: [{ "key": "default", "name": "Default Theme" }, ...]
 * Returns array of themes from the state JSON.
 */
export function get_theme_list_json(state_json: string): string;

export function get_version(): string;

/**
 * Register a theme from JSON. On duplicate, replace the theme's selectors, inheritance, and variables.
 * Expected JSON format: `{ "name": "theme-name", "theme": { "inherits": "parent", "selectors": {...}, "variables": {...}, "breakpoints": {...} } }`
 * Returns the updated state as JSON, or "{}" on error.
 */
export function register_theme_json(state_json: string, theme_json: string): string;

export function render_css_for_web(state_json: string): string;

/**
 * Set the default and current theme. Returns the updated state as JSON.
 */
export function set_theme_json(state_json: string, theme_name: string): string;

export type InitInput = RequestInfo | URL | Response | BufferSource | WebAssembly.Module;

export interface InitOutput {
  readonly memory: WebAssembly.Memory;
  readonly get_android_styles: (a: number, b: number, c: number, d: number, e: number, f: number, g: number) => void;
  readonly get_default_state_json: (a: number) => void;
  readonly get_theme_list_json: (a: number, b: number, c: number) => void;
  readonly get_version: (a: number) => void;
  readonly register_theme_json: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly render_css_for_web: (a: number, b: number, c: number) => void;
  readonly set_theme_json: (a: number, b: number, c: number, d: number, e: number) => void;
  readonly themed_styler_free_string: (a: number) => void;
  readonly themed_styler_render_css: (a: number) => number;
  readonly themed_styler_version: () => number;
  readonly __wbindgen_add_to_stack_pointer: (a: number) => number;
  readonly __wbindgen_export: (a: number, b: number) => number;
  readonly __wbindgen_export2: (a: number, b: number, c: number, d: number) => number;
  readonly __wbindgen_export3: (a: number, b: number, c: number) => void;
}

export type SyncInitInput = BufferSource | WebAssembly.Module;

/**
* Instantiates the given `module`, which can either be bytes or
* a precompiled `WebAssembly.Module`.
*
* @param {{ module: SyncInitInput }} module - Passing `SyncInitInput` directly is deprecated.
*
* @returns {InitOutput}
*/
export function initSync(module: { module: SyncInitInput } | SyncInitInput): InitOutput;

/**
* If `module_or_path` is {RequestInfo} or {URL}, makes a request and
* for everything else, calls `WebAssembly.instantiate` directly.
*
* @param {{ module_or_path: InitInput | Promise<InitInput> }} module_or_path - Passing `InitInput` directly is deprecated.
*
* @returns {Promise<InitOutput>}
*/
export default function __wbg_init (module_or_path?: { module_or_path: InitInput | Promise<InitInput> } | InitInput | Promise<InitInput>): Promise<InitOutput>;
