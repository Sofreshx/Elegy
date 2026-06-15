//! Normalized graph intermediate representation types.
//!
//! Mirrors the governed schema at `contracts/schemas/elegy-codegraph.graph.v0.json`.

use serde::{Deserialize, Serialize};

/// Content-addressable entity ID (SHA-1 hex string).
pub type EntityId = String;

/// Source location span — `[line, column]` pairs (1-indexed).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Span {
    pub start: (u32, u32),
    pub end: (u32, u32),
}

/// Optional type annotation on an input or output.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct TypeHint {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub type_hint: Option<String>,
}

/// Kind of code entity.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EntityKind {
    File,
    Module,
    Function,
    Class,
    Method,
    Trait,
    Impl,
    Interface,
    Type,
    Constant,
    Enum,
    Macro,
    Test,
    Doc,
}

/// Closed enumeration of observable side effects.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum SideEffect {
    #[serde(rename = "fs.read")]
    FsRead,
    #[serde(rename = "fs.write")]
    FsWrite,
    #[serde(rename = "net.http")]
    NetHttp,
    #[serde(rename = "net.grpc")]
    NetGrpc,
    #[serde(rename = "process.exec")]
    ProcessExec,
    #[serde(rename = "db.read")]
    DbRead,
    #[serde(rename = "db.write")]
    DbWrite,
    #[serde(rename = "env.read")]
    EnvRead,
    #[serde(rename = "env.write")]
    EnvWrite,
    #[serde(rename = "os.signal")]
    OsSignal,
}

/// Confidence level of a fact.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Confidence {
    Exact,
    Inferred,
    Heuristic,
}

/// Provenance metadata — required on every entity and edge.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Provenance {
    pub extractor: String,
    pub confidence: Confidence,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub evidence_refs: Vec<String>,
}

/// A node in the code graph.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Entity {
    pub id: EntityId,
    pub kind: EntityKind,
    pub layer: String,
    pub name: String,
    pub qualified_name: String,
    pub file: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub span: Option<Span>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub inputs: Vec<TypeHint>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub outputs: Vec<TypeHint>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub side_effects: Vec<SideEffect>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub dependencies: Vec<EntityId>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub tests: Vec<EntityId>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub docs: Vec<EntityId>,
    pub provenance: Provenance,
}

/// Kind of relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Imports,
    Exports,
    Calls,
    References,
    Reads,
    Writes,
    Validates,
    Emits,
    Owns,
    Tests,
    Documents,
}

/// A directed relationship between two entities.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Edge {
    pub src: EntityId,
    pub dst: EntityId,
    pub kind: EdgeKind,
    pub provenance: Provenance,
}

/// The complete graph extraction result.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Graph {
    pub schema: String,
    pub extractor: ExtractorMeta,
    pub entities: Vec<Entity>,
    pub edges: Vec<Edge>,
}

/// Extractor metadata.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ExtractorMeta {
    pub name: String,
    pub version: String,
    pub lang: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub warning: Option<String>,
}
