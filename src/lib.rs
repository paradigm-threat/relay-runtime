//! relay-runtime: Universal client hook runtime
//!
//! Parses, executes, and renders client hooks (JS-like + JSX) for web and native.
//!
//! See docs/PLAN.md and docs/IMPLEMENTATION_PHASES.md.

pub mod analysis;
pub mod ast;
pub mod error;
pub mod hook_runner;
pub mod host;
pub mod html;
pub mod lexer;
pub mod markdown_host;
pub mod relay_modules;
pub mod session_theme;
pub mod style_host;
pub mod video_host;
pub mod parser;
pub mod run_state;
pub mod runtime;
pub mod server_script;
pub mod style_resolve;
pub mod desktop_style;
pub mod value;
pub mod vdom;

#[cfg(target_arch = "wasm32")]
pub mod wasm_api;

#[cfg(target_os = "android")]
pub mod android_jni;

pub use analysis::{analyze, AnalysisResult, ExportInfo, HookCallSite};
pub use ast::Program;
pub use error::{ParseErrorDetail, RuntimeError, RuntimeErrorDetail, SourceLoc};
pub use parser::{parse, parse_with_path};
pub use host::{fetch_options_json, host_fetch_object, Host, HostHandle, MockHost};
pub use hook_runner::{
    reset_hook_session, run_hook_to_vnode, run_hook_to_vnode_stable,
    run_hook_to_vnode_stable_isolated, HookRunResult,
};
pub use html::{
    escape_html_text, escape_json_for_html_script, vnode_to_html, HtmlRenderOptions,
};
pub use run_state::{
    clear_run_state, clear_handler_registry_for_vnode_pass, clear_tip_get_cache, consume_render_requested, invoke_handler,
    invoke_handler_value, register_handler, run_hook_persistent, set_host_for_lazy, tip_cached_get,
    with_isolated_hook_run,
};
pub use session_theme::{
    clear_session_theme, effective_theme_yaml, get_session_theme_yaml, looks_like_theme_yaml,
    set_session_theme_yaml,
};
pub use style_host::{load_style_exports, style_internal_module, try_load_style_module};
pub use server_script::run_query_script;
pub use runtime::Runtime;
pub use value::{NativeFnClosure, Value};
pub use vdom::{value_to_vnode, PropValue, VNode};
