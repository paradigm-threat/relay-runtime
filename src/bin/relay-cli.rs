//! relay-runtime CLI: compile and validate hooks
//!
//! Recursively finds .jsx/.js files, compiles them, and reports errors/warnings
//! in an ESLint-style format.

use relay_runtime::{analyze, parse, RuntimeError};
use relay_runtime::ast::Statement;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::fs;
use std::io::{self, Write};

const EXTENSIONS: &[&str] = &["jsx", "js"];

#[derive(Debug)]
struct Diag {
    file: PathBuf,
    line: usize,
    col: usize,
    msg: String,
    severity: Severity,
    code: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum Severity {
    Error,
    Warning,
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let dir = args.get(1).map(Path::new).unwrap_or_else(|| Path::new("."));
    if !dir.is_dir() {
        eprintln!("Usage: relay-cli [directory]");
        eprintln!("  directory: path to recursively scan for .jsx/.js files (default: .)");
        std::process::exit(1);
    }

    let mut diags = Vec::new();
    let files = collect_files(dir);
    let mut file_to_path: HashMap<String, PathBuf> = HashMap::new();

    for path in &files {
        let rel = path.strip_prefix(dir).unwrap_or(path);
        file_to_path.insert(rel.to_string_lossy().replace('\\', "/"), path.clone());
    }

    for path in &files {
        let rel = path.strip_prefix(dir).unwrap_or(path);
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        match fs::read_to_string(path) {
            Ok(source) => {
                let mut file_diags = compile_file(path, &source, &rel_str);
                for d in &mut file_diags {
                    d.file = rel.to_path_buf();
                }
                diags.extend(file_diags);
            }
            Err(e) => {
                diags.push(Diag {
                    file: rel.to_path_buf(),
                    line: 0,
                    col: 0,
                    msg: format!("Failed to read file: {}", e),
                    severity: Severity::Error,
                    code: None,
                });
            }
        }
    }

    // Validate imports
    for path in &files {
        let rel = path.strip_prefix(dir).unwrap_or(path);
        if let Ok(source) = fs::read_to_string(path) {
            let import_diags = validate_imports(path, &source, &file_to_path);
            for mut d in import_diags {
                d.file = rel.to_path_buf();
                diags.push(d);
            }
        }
    }

    let errors = diags.iter().filter(|d| d.severity == Severity::Error).count();
    let warnings = diags.iter().filter(|d| d.severity == Severity::Warning).count();

    for d in &diags {
        let sev = match d.severity {
            Severity::Error => "error",
            Severity::Warning => "warning",
        };
        let code = d
            .code
            .as_deref()
            .map(|c| format!(" ({})", c))
            .unwrap_or_default();
        writeln!(
            io::stderr(),
            "{}:{}:{}: {}: {}{}",
            d.file.display(),
            d.line,
            d.col,
            sev,
            d.msg,
            code
        )
        .ok();
    }

    if errors > 0 {
        writeln!(io::stderr(), "\n{} error(s), {} warning(s)", errors, warnings).ok();
        std::process::exit(1);
    }
    if warnings > 0 {
        writeln!(io::stderr(), "\n{} warning(s)", warnings).ok();
    }
}

fn collect_files(dir: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(entries) = fs::read_dir(dir) {
            for e in entries.flatten() {
                let p = e.path();
                if p.is_dir() {
                    let name = p.file_name().unwrap_or_default();
                    let s = name.to_string_lossy();
                    let skip = s == "node_modules"
                        || s.starts_with('.')
                        || s == "pkg"
                        || s == "themed-styler";
                    if !skip {
                        walk(&p, out);
                    }
                } else if let Some(ext) = p.extension() {
                    if EXTENSIONS.contains(&ext.to_string_lossy().as_ref()) {
                        out.push(p);
                    }
                }
            }
        }
    }
    walk(dir, &mut out);
    out
}

fn line_col_for_offset(source: &str, offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, c) in source.chars().enumerate() {
        if i >= offset {
            break;
        }
        if c == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn compile_file(path: &Path, source: &str, _rel: &str) -> Vec<Diag> {
    let mut diags = Vec::new();

    match parse(source) {
        Ok(program) => {
            match analyze(&program) {
                Ok(analysis) => {
                    if analysis.default_export.is_none() && !program.body.is_empty() {
                        let has_export = program.body.iter().any(|s| {
                            matches!(s, Statement::ExportDefault { .. })
                        });
                        if !has_export {
                            diags.push(Diag {
                                file: path.to_path_buf(),
                                line: 1,
                                col: 1,
                                msg: "No default export found (expected for hook entry)".to_string(),
                                severity: Severity::Warning,
                                code: Some("no-default-export".to_string()),
                            });
                        }
                    }
                }
                Err(e) => {
                    diags.push(Diag {
                        file: path.to_path_buf(),
                        line: 1,
                        col: 1,
                        msg: format!("Analysis error: {}", e),
                        severity: Severity::Error,
                        code: None,
                    });
                }
            }
        }
        Err(e) => {
            let (line, col) = parse_error_location(&e, source);
            diags.push(Diag {
                file: path.to_path_buf(),
                line,
                col,
                msg: format!("{}", e),
                severity: Severity::Error,
                code: Some("parse-error".to_string()),
            });
        }
    }

    diags
}

fn parse_error_location(_e: &RuntimeError, _source: &str) -> (usize, usize) {
    // Parse error messages often don't include position; default to line 1
    (1, 1)
}

fn validate_imports(
    path: &Path,
    source: &str,
    file_to_path: &HashMap<String, PathBuf>,
) -> Vec<Diag> {
    let mut diags = Vec::new();
    let base = path.parent().unwrap_or(Path::new("."));
    if let Ok(program) = parse(source) {
        for stmt in &program.body {
            if let Statement::ImportDecl { source: spec, .. } = stmt {
                if is_builtin_spec(spec) {
                    continue;
                }
                if let Some(rel) = resolve_import_spec(spec, base) {
                    let full = base.join(&rel);
                    if !full.exists() {
                        let found = file_to_path.keys().any(|k| {
                            k == &rel || k.ends_with(&format!("/{}", rel)) || rel.ends_with(k)
                        });
                        if !found {
                            diags.push(Diag {
                                file: path.to_path_buf(),
                                line: 1,
                                col: 1,
                                msg: format!("Import path not found: {} (resolved: {})", spec, rel),
                                severity: Severity::Warning,
                                code: Some("import-not-found".to_string()),
                            });
                        }
                    }
                }
            }
        }
    }
    diags
}

fn resolve_import_spec(spec: &str, base: &Path) -> Option<String> {
    if spec.starts_with('.') {
        let mut p = base.to_path_buf();
        for part in spec.split('/') {
            match part {
                "." => {}
                ".." => {
                    p.pop();
                }
                _ => p.push(part),
            }
        }
        if let Ok(rel) = p.strip_prefix(base) {
            Some(rel.to_string_lossy().replace('\\', "/"))
        } else {
            Some(spec.to_string())
        }
    } else {
        None
    }
}

fn is_builtin_spec(spec: &str) -> bool {
    spec == "react"
        || spec.starts_with("react/")
        || spec == "@theme"
        || spec.starts_with("@theme/")
        || spec == "@hooks"
        || spec.starts_with("@hooks/")
        || spec.starts_with("@clevertree/")
        || spec.starts_with("relay-runtime")
}
