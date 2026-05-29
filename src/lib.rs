//! relay-runtime: Universal client hook runtime
//!
//! Parses, executes, and renders client hooks (JS-like + JSX) for web and native.
//!
//! See docs/PLAN.md and docs/IMPLEMENTATION_PHASES.md.

pub mod analysis;
pub mod ast;
pub mod error;
pub mod host;
pub mod lexer;
pub mod markdown_host;
pub mod parser;
pub mod run_state;
pub mod runtime;
pub mod server_script;
pub mod style_resolve;
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
pub use run_state::{
    clear_run_state, clear_handler_registry_for_vnode_pass, consume_render_requested, invoke_handler,
    invoke_handler_value, register_handler, run_hook_persistent,
};
pub use server_script::run_query_script;
pub use runtime::Runtime;
pub use value::{NativeFnClosure, Value};
pub use vdom::{value_to_vnode, PropValue, VNode};
