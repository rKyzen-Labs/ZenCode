use std::path::Path;

/// Shreds file content to an AST-level structural outline.
/// Strips implementation details and function/method bodies while preserving
/// signatures, imports, structs, interfaces, type definitions, and structure.
pub fn shred_file_context(path: &Path, content: &str) -> String {
    let ext = path
        .extension()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_lowercase();

    match ext.as_str() {
        "rs" => shred_rust(content),
        "py" => shred_python(content),
        "ts" | "tsx" | "js" | "jsx" => shred_typescript(content),
        "go" => shred_go(content),
        "toml" | "json" | "yaml" | "yml" => shred_config(content),
        _ => shred_generic(content),
    }
}

/// Shreds Rust source code: preserves imports, struct/enum/type/trait definitions,
/// and replaces function/method bodies with `/* [shredded body] */`.
fn shred_rust(content: &str) -> String {
    let mut out = Vec::new();
    let mut in_function = false;
    let mut brace_depth = 0i32;

    for line in content.lines() {
        let trimmed = line.trim();

        // Preserve attributes, use statements, structs, enums, traits, types, consts
        if !in_function {
            if trimmed.starts_with("use ")
                || trimmed.starts_with("pub use ")
                || trimmed.starts_with("#[")
                || trimmed.starts_with("mod ")
                || trimmed.starts_with("pub mod ")
                || trimmed.starts_with("struct ")
                || trimmed.starts_with("pub struct ")
                || trimmed.starts_with("enum ")
                || trimmed.starts_with("pub enum ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("pub type ")
                || trimmed.starts_with("trait ")
                || trimmed.starts_with("pub trait ")
                || trimmed.starts_with("impl ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("pub const ")
                || trimmed.starts_with("static ")
                || trimmed.starts_with("pub static ")
                || trimmed.starts_with("//")
                || trimmed.starts_with("///")
                || trimmed.starts_with("/*")
            {
                out.push(line.to_string());
                continue;
            }

            // Check if line defines a function
            if trimmed.starts_with("fn ")
                || trimmed.starts_with("pub fn ")
                || trimmed.starts_with("pub(crate) fn ")
                || trimmed.starts_with("async fn ")
                || trimmed.starts_with("pub async fn ")
                || trimmed.starts_with("pub(crate) async fn ")
            {
                if let Some(brace_pos) = line.find('{') {
                    // One-line or multi-line function opening
                    let signature = &line[..brace_pos];
                    out.push(format!("{signature}{{ /* [shredded body] */ }}"));
                    // Check if closed on same line
                    let open_braces = line.matches('{').count() as i32;
                    let close_braces = line.matches('}').count() as i32;
                    if open_braces > close_braces {
                        in_function = true;
                        brace_depth = open_braces - close_braces;
                    }
                } else {
                    // Signature spans multiple lines before opening brace
                    out.push(line.to_string());
                    in_function = true;
                    brace_depth = 0;
                }
                continue;
            }

            out.push(line.to_string());
        } else {
            // Inside function body
            let open_braces = line.matches('{').count() as i32;
            let close_braces = line.matches('}').count() as i32;
            brace_depth += open_braces - close_braces;

            if brace_depth <= 0 {
                in_function = false;
                brace_depth = 0;
            }
        }
    }

    out.join("\n")
}

/// Shreds Python code: preserves imports, class headers, and replaces def bodies with pass.
fn shred_python(content: &str) -> String {
    let mut out = Vec::new();
    let mut in_def = false;
    let mut def_indent = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            out.push(line.to_string());
            continue;
        }

        let current_indent = line.len() - line.trim_start().len();

        if in_def {
            if current_indent > def_indent {
                // Skip function body lines
                continue;
            } else {
                in_def = false;
            }
        }

        if trimmed.starts_with("import ")
            || trimmed.starts_with("from ")
            || trimmed.starts_with("class ")
            || trimmed.starts_with("@")
            || trimmed.starts_with("#")
        {
            out.push(line.to_string());
            continue;
        }

        if trimmed.starts_with("def ") || trimmed.starts_with("async def ") {
            out.push(line.to_string());
            out.push(format!("{}    pass  # [shredded body]", " ".repeat(current_indent)));
            in_def = true;
            def_indent = current_indent;
            continue;
        }

        // Top-level variable assignments or docstrings
        if current_indent == 0 {
            out.push(line.to_string());
        }
    }

    out.join("\n")
}

/// Shreds TypeScript/JavaScript code: preserves interfaces, types, exports,
/// and replaces function and method bodies with `/* [shredded body] */`.
fn shred_typescript(content: &str) -> String {
    let mut out = Vec::new();
    let mut in_function = false;
    let mut brace_depth = 0i32;

    for line in content.lines() {
        let trimmed = line.trim();

        if !in_function {
            if trimmed.starts_with("import ")
                || trimmed.starts_with("export interface ")
                || trimmed.starts_with("interface ")
                || trimmed.starts_with("export type ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("export const ")
                || trimmed.starts_with("//")
                || trimmed.starts_with("/*")
            {
                out.push(line.to_string());
                continue;
            }

            if trimmed.starts_with("function ")
                || trimmed.starts_with("export function ")
                || trimmed.starts_with("export default function ")
                || trimmed.starts_with("async function ")
                || trimmed.starts_with("export async function ")
            {
                if let Some(brace_pos) = line.find('{') {
                    let signature = &line[..brace_pos];
                    out.push(format!("{signature}{{ /* [shredded body] */ }}"));
                    let open_braces = line.matches('{').count() as i32;
                    let close_braces = line.matches('}').count() as i32;
                    if open_braces > close_braces {
                        in_function = true;
                        brace_depth = open_braces - close_braces;
                    }
                } else {
                    out.push(line.to_string());
                    in_function = true;
                    brace_depth = 0;
                }
                continue;
            }

            out.push(line.to_string());
        } else {
            let open_braces = line.matches('{').count() as i32;
            let close_braces = line.matches('}').count() as i32;
            brace_depth += open_braces - close_braces;

            if brace_depth <= 0 {
                in_function = false;
                brace_depth = 0;
            }
        }
    }

    out.join("\n")
}

/// Shreds Go code: preserves packages, imports, structs, interfaces, and function signatures.
fn shred_go(content: &str) -> String {
    let mut out = Vec::new();
    let mut in_function = false;
    let mut brace_depth = 0i32;

    for line in content.lines() {
        let trimmed = line.trim();

        if !in_function {
            if trimmed.starts_with("package ")
                || trimmed.starts_with("import ")
                || trimmed.starts_with("type ")
                || trimmed.starts_with("const ")
                || trimmed.starts_with("var ")
                || trimmed.starts_with("//")
            {
                out.push(line.to_string());
                continue;
            }

            if trimmed.starts_with("func ") {
                if let Some(brace_pos) = line.find('{') {
                    let signature = &line[..brace_pos];
                    out.push(format!("{signature}{{ /* [shredded body] */ }}"));
                    let open_braces = line.matches('{').count() as i32;
                    let close_braces = line.matches('}').count() as i32;
                    if open_braces > close_braces {
                        in_function = true;
                        brace_depth = open_braces - close_braces;
                    }
                } else {
                    out.push(line.to_string());
                    in_function = true;
                    brace_depth = 0;
                }
                continue;
            }

            out.push(line.to_string());
        } else {
            let open_braces = line.matches('{').count() as i32;
            let close_braces = line.matches('}').count() as i32;
            brace_depth += open_braces - close_braces;

            if brace_depth <= 0 {
                in_function = false;
                brace_depth = 0;
            }
        }
    }

    out.join("\n")
}

/// Shreds config files by keeping top section headers and key outlines.
fn shred_config(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 60 {
        content.to_string()
    } else {
        let mut truncated = lines[..50].join("\n");
        truncated.push_str("\n... [config structure truncated for token conservation] ...");
        truncated
    }
}

/// Generic fallback for text or unknown files.
fn shred_generic(content: &str) -> String {
    let lines: Vec<&str> = content.lines().collect();
    if lines.len() <= 40 {
        content.to_string()
    } else {
        let mut truncated = lines[..30].join("\n");
        truncated.push_str("\n... [content truncated for token conservation] ...");
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shred_rust_function_bodies() {
        let src = r#"use std::path::Path;

pub struct Config {
    pub timeout: u64,
}

pub fn calculate_sum(a: i32, b: i32) -> i32 {
    let mut x = a;
    x += b;
    x
}

impl Config {
    pub fn new() -> Self {
        Self { timeout: 30 }
    }
}
"#;
        let shredded = shred_file_context(Path::new("test.rs"), src);
        assert!(shredded.contains("use std::path::Path;"));
        assert!(shredded.contains("pub struct Config"));
        assert!(shredded.contains("calculate_sum(a: i32, b: i32) -> i32 { /* [shredded body] */ }"));
        assert!(!shredded.contains("let mut x = a;"));
    }

    #[test]
    fn test_shred_python_def_bodies() {
        let src = r#"import os

DEFAULT_VALUE = 42

class Calculator:
    def add(self, x, y):
        print("adding numbers")
        return x + y

def main():
    c = Calculator()
    print(c.add(1, 2))
"#;
        let shredded = shred_file_context(Path::new("calc.py"), src);
        assert!(shredded.contains("import os"));
        assert!(shredded.contains("class Calculator:"));
        assert!(shredded.contains("pass  # [shredded body]"));
        assert!(!shredded.contains("print(\"adding numbers\")"));
    }

    #[test]
    fn test_shred_typescript_functions() {
        let src = r#"import { useState } from 'react';

export interface User {
    id: string;
    name: string;
}

export function getUser(id: string): User {
    const raw = fetchUser(id);
    return JSON.parse(raw);
}
"#;
        let shredded = shred_file_context(Path::new("user.ts"), src);
        assert!(shredded.contains("import { useState }"));
        assert!(shredded.contains("export interface User"));
        assert!(shredded.contains("getUser(id: string): User { /* [shredded body] */ }"));
        assert!(!shredded.contains("const raw = fetchUser(id);"));
    }
}
