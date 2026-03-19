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
pub mod parser;
pub mod runtime;
pub mod value;
pub mod vdom;

#[cfg(target_arch = "wasm32")]
pub mod wasm_api;

pub use analysis::{analyze, AnalysisResult, ExportInfo, HookCallSite};
pub use ast::Program;
pub use error::{ParseErrorDetail, RuntimeError, SourceLoc};
pub use parser::{parse, parse_with_path};
pub use host::{Host, HostHandle, MockHost};
pub use runtime::Runtime;
pub use value::Value;
pub use vdom::{value_to_vnode, PropValue, VNode};
