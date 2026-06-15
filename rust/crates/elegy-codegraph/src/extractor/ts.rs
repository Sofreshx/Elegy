//! TypeScript extractor using the TypeScript Compiler API.
//!
//! Extracts files, modules, exports, symbols, calls, tests, and doc links
//! from TypeScript source code.
//!
//! ## Known gaps (v0)
//! - Cross-package monorepo resolution is best-effort via tsconfig paths/references.
//! - No runtime test-to-source binding; test detection is pattern-based.
//! - Dynamic `import()` expressions are only traced for literal string arguments.

use crate::ir::Graph;

/// Extract a graph from a TypeScript project at the given path.
pub fn extract(_repo_path: &str) -> crate::error::Result<Graph> {
    unimplemented!("TypeScript Compiler API extraction (wp-ts-extract)")
}
