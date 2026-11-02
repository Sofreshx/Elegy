//! Non-interactive project-resolution check for `elegy-run`.
//!
//! `--preflight` resolves the project and composes its runtime state exactly
//! like the stdio host does at startup, but never opens the MCP transport.
//! It exists so a broken descriptor configuration (e.g. `DESC-CONFIG-002`)
//! fails a local check or a CI job loudly, instead of only surfacing later as
//! a silent MCP reconnect loop in a host like Claude Desktop.

use elegy_core::{compose_runtime_state, CoreError, ProjectLocator};

/// Result of resolving and composing a project without serving it.
pub enum PreflightOutcome {
    /// The project's descriptors loaded and at least the reported number of
    /// resources were composed into the catalog.
    Ready {
        project_name: String,
        resource_count: usize,
    },
    /// Project resolution or descriptor composition failed. The MCP host
    /// would still start in this case (see `ElegyMcpHost::degraded`), but a
    /// preflight check treats it as a failure so it can be caught early.
    Degraded(CoreError),
}

/// Resolve and compose `locator` without serving it over any transport.
pub fn run_preflight(locator: ProjectLocator) -> PreflightOutcome {
    match compose_runtime_state(locator) {
        Ok(state) => {
            let catalog = state.catalog();
            PreflightOutcome::Ready {
                project_name: catalog.project_name.clone(),
                resource_count: catalog.resource_count,
            }
        }
        Err(error) => PreflightOutcome::Degraded(error),
    }
}

impl PreflightOutcome {
    /// Print a human-readable report to stderr and return the process exit
    /// code: `0` when the project resolved cleanly, `1` when it is degraded.
    pub fn report(&self) -> i32 {
        match self {
            PreflightOutcome::Ready {
                project_name,
                resource_count,
            } => {
                eprintln!(
                    "elegy-run preflight OK: project {project_name:?} resolved {resource_count} resource(s)."
                );
                0
            }
            PreflightOutcome::Degraded(error) => {
                eprintln!(
                    "elegy-run preflight FAILED: descriptor configuration did not load; \
                     the MCP host would start in degraded mode."
                );
                for diagnostic in error.diagnostics() {
                    let path = diagnostic
                        .location
                        .path
                        .as_deref()
                        .map(|value| format!(" (path: {value})"))
                        .unwrap_or_default();
                    let field = diagnostic
                        .location
                        .field
                        .as_deref()
                        .map(|value| format!(" (field: {value})"))
                        .unwrap_or_default();
                    eprintln!(
                        "  [{}] {}{path}{field}",
                        diagnostic.code, diagnostic.message
                    );
                }
                1
            }
        }
    }
}
