//! Host bindings: fetch, navigate, createElement, load_module.
//!
//! The host provides platform-specific implementations. The runtime
//! dispatches calls to these when the script invokes fetch(), navigate(), etc.

use crate::value::Value;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

/// Handle to host implementation.
pub type HostHandle = std::rc::Rc<std::cell::RefCell<dyn Host>>;

/// Host interface for platform bindings.
pub trait Host {
    /// fetch(url, options?) -> response-like object
    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String>;

    /// navigate(path) -> ()
    fn navigate(&self, path: &str) -> Result<(), String>;

    /// createElement(tag, props, children) -> element descriptor
    fn create_element(
        &self,
        tag: &str,
        props: &Value,
        children: &[Value],
    ) -> Result<Value, String>;

    /// import(path) -> module object with default, named exports
    fn load_module(&self, path: &str) -> Result<Value, String>;
}

/// Mock host for testing. Records all calls.
#[derive(Debug, Default)]
pub struct MockHost {
    pub fetch_calls: RefCell<Vec<(String, Option<Value>)>>,
    pub navigate_calls: RefCell<Vec<String>>,
    pub create_element_calls: RefCell<Vec<(String, Value, Vec<Value>)>>,
    pub load_module_calls: RefCell<Vec<String>>,
    /// Modules to return from load_module: path -> module (Object with default, etc.)
    pub modules: RefCell<std::collections::HashMap<String, Value>>,
}

impl MockHost {
    pub fn new() -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self::default()))
    }

    /// Register a module to return from load_module.
    pub fn register_module(&self, path: &str, module: Value) {
        self.modules.borrow_mut().insert(path.to_string(), module);
    }
}

impl Host for MockHost {
    fn fetch(&self, url: &str, options: Option<&Value>) -> Result<Value, String> {
        self.fetch_calls
            .borrow_mut()
            .push((url.to_string(), options.cloned()));
        Ok(Value::Object(
            [
                ("ok".to_string(), Value::Bool(true)),
                ("status".to_string(), Value::Number(200.0)),
            ]
            .into_iter()
            .collect(),
        ))
    }

    fn navigate(&self, path: &str) -> Result<(), String> {
        self.navigate_calls.borrow_mut().push(path.to_string());
        Ok(())
    }

    fn create_element(
        &self,
        tag: &str,
        props: &Value,
        children: &[Value],
    ) -> Result<Value, String> {
        self.create_element_calls
            .borrow_mut()
            .push((tag.to_string(), props.clone(), children.to_vec()));
        Ok(Value::Object(
            [
                ("type".to_string(), Value::String("element".to_string())),
                ("tag".to_string(), Value::String(tag.to_string())),
                ("props".to_string(), props.clone()),
                (
                    "children".to_string(),
                    Value::Array(children.to_vec()),
                ),
            ]
            .into_iter()
            .collect(),
        ))
    }

    fn load_module(&self, path: &str) -> Result<Value, String> {
        self.load_module_calls.borrow_mut().push(path.to_string());
        self.modules
            .borrow()
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Module not found: {}", path))
    }
}

/// Host that loads modules from a path->source map. Used for dynamic import() in WASM.
/// Resolves path to source, parses, and returns module exports (default export function).
/// When loading child modules, uses parent_host (if set) so child can resolve imports like 'react'.
pub struct ModuleMapHost {
    modules: HashMap<String, String>,
    cache: RefCell<HashMap<String, Value>>,
    parent_host: RefCell<Option<HostHandle>>,
}

impl ModuleMapHost {
    pub fn new(modules: HashMap<String, String>) -> Rc<RefCell<Self>> {
        Rc::new(RefCell::new(Self {
            modules,
            cache: RefCell::new(HashMap::new()),
            parent_host: RefCell::new(None),
        }))
    }

    pub fn add_module(&mut self, path: impl Into<String>, source: impl Into<String>) {
        self.modules.insert(path.into(), source.into());
    }

    /// Set the parent host for resolving child module imports (e.g. 'react').
    pub fn set_parent_host(&self, host: HostHandle) {
        *self.parent_host.borrow_mut() = Some(host);
    }
}

impl Host for ModuleMapHost {
    fn fetch(&self, _url: &str, _options: Option<&Value>) -> Result<Value, String> {
        Err("fetch not implemented in ModuleMapHost".to_string())
    }

    fn navigate(&self, _path: &str) -> Result<(), String> {
        Err("navigate not implemented in ModuleMapHost".to_string())
    }

    fn create_element(
        &self,
        _tag: &str,
        _props: &Value,
        _children: &[Value],
    ) -> Result<Value, String> {
        Err("createElement not implemented in ModuleMapHost".to_string())
    }

    fn load_module(&self, path: &str) -> Result<Value, String> {
        if let Some(cached) = self.cache.borrow().get(path).cloned() {
            return Ok(cached);
        }
        let source = self
            .modules
            .get(path)
            .cloned()
            .ok_or_else(|| format!("Module not found: {}", path))?;
        let program = crate::parse(&source).map_err(|e| format!("Parse error: {}", e))?;
        let analysis = crate::analyze(&program).map_err(|e| format!("Analysis error: {}", e))?;
        let mut runtime = crate::Runtime::new(program, analysis);
        if let Some(ref host) = *self.parent_host.borrow() {
            runtime = runtime.with_host(host.clone());
        }
        let exports = runtime.get_module_exports_for_path(path).map_err(|e| format!("{}", e))?;
        self.cache.borrow_mut().insert(path.to_string(), exports.clone());
        Ok(exports)
    }
}

