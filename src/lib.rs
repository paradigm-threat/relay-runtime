//! relay-runtime: Universal client hook runtime
//!
//! Parses, executes, and renders client hooks (JS-like + JSX) for web and native.
//!
//! See docs/POLICY.md, docs/CLEVERSCRIPT.md, docs/TERMINOLOGY.md, docs/PERF_FEATURE_FLAGS.md,
//! and docs/ANDROID_WEB_TEST.md.

pub mod analysis;
pub mod ast;
pub mod error;
pub mod hook_runner;
pub mod host;
pub mod html;
pub mod lexer;
pub mod markdown_host;
pub mod nav_intent;
pub mod perf_flags;
pub mod relay_modules;
pub mod session_theme;
pub mod ssr_cancel;
pub mod boot_shell_host;
pub mod style_host;
pub mod video_host;
pub mod parser;
pub mod run_state;
pub mod runtime;
pub mod server_script;
pub mod style_resolve;
pub mod desktop_style;
pub mod dom_host;
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
    clear_cs_global_this_extras, clear_run_state, clear_handler_registry_for_vnode_pass,
    clear_tip_get_cache, consume_render_requested, invoke_handler, invoke_handler_value,
    register_handler, run_hook_persistent, set_cs_global_this_extra, set_host_for_lazy,
    tip_cached_get, tip_cached_nav_index_rows, with_isolated_hook_run,
};
pub use ssr_cancel::{
    install_ssr_cancel, signal_ssr_cancel, ssr_cancel_installed, ssr_is_cancelled,
};
pub use perf_flags::{
    array_host_builder, cached_get_prefer_markdown, clear_perf_flag_cache_for_tests,
    cs_cancel_in_loops, fuse_ssr_serialize, is_ssr_thread, md_cancel_before_parse,
    md_fragment_cache, md_nested_loud_fail, md_ssr_fast_path, md_ssr_skip_anchor_onclick,
    md_stub_overrides_noop,
};
pub use session_theme::{
    clear_session_theme, effective_theme_yaml, get_session_theme_yaml, looks_like_theme_yaml,
    set_session_theme_yaml,
};
pub use style_host::{load_style_exports, style_internal_module, try_load_style_module};
pub use boot_shell_host::{load_relay_boot_shell_exports, try_load_boot_shell_module};
pub use server_script::run_query_script;
pub use runtime::Runtime;
pub use value::{NativeFnClosure, Value};
pub use vdom::{value_to_vnode, PropValue, VNode};
