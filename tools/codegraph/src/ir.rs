//! Normalized graph intermediate representation types.
//!
//! Mirrors the governed schema at `schemas/elegy-codegraph.graph.v0.json`.

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

impl Entity {
    /// Compute a stable content-addressable ID from the entity's qualified name, file, and kind.
    ///
    /// Uses SHA-256 (first 40 hex chars) over a canonicalization of
    /// `qualifiedName|file|kind`.
    pub fn compute_id(qualified_name: &str, file: &str, kind: &EntityKind) -> EntityId {
        use sha2::{Digest, Sha256};
        let kind_str = serde_json::to_string(kind).unwrap_or_else(|_| format!("{:?}", kind));
        let input = format!("{}|{}|{}", qualified_name, file, kind_str);
        let hash = Sha256::digest(input.as_bytes());
        // Take first 20 bytes, format as 40-char hex string
        hash.iter().take(20).map(|b| format!("{:02x}", b)).collect()
    }

    /// Returns true if this entity is a test.
    pub fn is_test(&self) -> bool {
        self.kind == EntityKind::Test
    }

    /// Returns true if this entity lives in the test layer.
    pub fn in_test_layer(&self) -> bool {
        self.layer == "test"
    }

    /// Returns true if this entity has at least one documented side effect.
    pub fn has_side_effects(&self) -> bool {
        !self.side_effects.is_empty()
    }
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

impl Graph {
    /// Create a new empty graph with the given extractor metadata.
    pub fn new(extractor: ExtractorMeta) -> Self {
        Self {
            schema: "elegy-codegraph.graph.v0".to_string(),
            extractor,
            entities: Vec::new(),
            edges: Vec::new(),
        }
    }

    /// Add an entity to the graph.
    pub fn add_entity(&mut self, entity: Entity) {
        self.entities.push(entity);
    }

    /// Add an edge to the graph.
    pub fn add_edge(&mut self, edge: Edge) {
        self.edges.push(edge);
    }

    /// Find an entity by ID.
    pub fn find_entity(&self, id: &EntityId) -> Option<&Entity> {
        self.entities.iter().find(|e| &e.id == id)
    }

    /// Get all entities of a given kind.
    pub fn entities_of_kind(&self, kind: &EntityKind) -> Vec<&Entity> {
        self.entities.iter().filter(|e| &e.kind == kind).collect()
    }

    /// Get all edges of a given kind.
    pub fn edges_of_kind(&self, kind: &EdgeKind) -> Vec<&Edge> {
        self.edges.iter().filter(|e| &e.kind == kind).collect()
    }

    /// Get incoming edges to an entity.
    pub fn incoming_edges(&self, id: &EntityId) -> Vec<&Edge> {
        self.edges.iter().filter(|e| &e.dst == id).collect()
    }

    /// Get outgoing edges from an entity.
    pub fn outgoing_edges(&self, id: &EntityId) -> Vec<&Edge> {
        self.edges.iter().filter(|e| &e.src == id).collect()
    }

    /// Get total entity count.
    pub fn entity_count(&self) -> usize {
        self.entities.len()
    }

    /// Get total edge count.
    pub fn edge_count(&self) -> usize {
        self.edges.len()
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Load the contract fixture and verify serde round-trip.
    #[test]
    fn round_trip_contract_fixture() {
        // From CARGO_MANIFEST_DIR (tools/codegraph/), fixture is at ./fixtures/
        let fixture_path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/fixtures/elegy-codegraph.graph.v0.example.json"
        );
        let json = std::fs::read_to_string(fixture_path).expect("Failed to read contract fixture");

        // Deserialize
        let graph: Graph =
            serde_json::from_str(&json).expect("Failed to deserialize contract fixture");

        // Verify schema
        assert_eq!(graph.schema, "elegy-codegraph.graph.v0");

        // Verify extractor metadata
        assert_eq!(graph.extractor.name, "elegy-codegraph-ts");
        assert_eq!(graph.extractor.lang, "ts");

        // Verify entity count
        assert_eq!(graph.entities.len(), 3);
        assert_eq!(graph.edges.len(), 4);

        // Verify entity kinds
        assert_eq!(graph.entities[0].kind, EntityKind::Module);
        assert_eq!(graph.entities[1].kind, EntityKind::Function);
        assert_eq!(graph.entities[2].kind, EntityKind::Test);

        // Verify entity layers
        assert_eq!(graph.entities[0].layer, "source");
        assert_eq!(graph.entities[2].layer, "test");

        // Verify provenance on first entity
        assert_eq!(graph.entities[0].provenance.confidence, Confidence::Exact);
        assert!(!graph.entities[0].provenance.evidence_refs.is_empty());

        // Verify test entity has inferred confidence
        assert_eq!(
            graph.entities[2].provenance.confidence,
            Confidence::Inferred
        );

        // Verify edge kinds
        assert_eq!(graph.edges[0].kind, EdgeKind::Exports);
        assert_eq!(graph.edges[1].kind, EdgeKind::Calls);
        assert_eq!(graph.edges[2].kind, EdgeKind::Tests);
        assert_eq!(graph.edges[3].kind, EdgeKind::Documents);

        // Verify edge provenance
        assert_eq!(graph.edges[3].provenance.confidence, Confidence::Heuristic);

        // Serialize back and verify it produces valid JSON
        let re_serialized =
            serde_json::to_string_pretty(&graph).expect("Failed to re-serialize graph");
        let _reparsed: serde_json::Value =
            serde_json::from_str(&re_serialized).expect("Re-serialized output is not valid JSON");

        // Verify key fields survive round-trip
        let round_trip: Graph =
            serde_json::from_str(&re_serialized).expect("Round-trip deserialization failed");
        assert_eq!(round_trip.entities.len(), 3);
        assert_eq!(round_trip.edges.len(), 4);
    }

    #[test]
    fn entity_compute_id_is_stable() {
        let id1 = Entity::compute_id("src/math::add", "src/math.ts", &EntityKind::Function);
        let id2 = Entity::compute_id("src/math::add", "src/math.ts", &EntityKind::Function);
        assert_eq!(id1, id2);
        assert_eq!(id1.len(), 40);

        // Different qualified name should produce different ID
        let id3 = Entity::compute_id("src/math::subtract", "src/math.ts", &EntityKind::Function);
        assert_ne!(id1, id3);
    }

    #[test]
    fn graph_builder_methods() {
        let meta = ExtractorMeta {
            name: "test-extractor".into(),
            version: "0.1.0".into(),
            lang: "ts".into(),
            warning: None,
        };
        let mut graph = Graph::new(meta);

        let entity = Entity {
            id: "test-id".into(),
            kind: EntityKind::Function,
            layer: "source".into(),
            name: "test_fn".into(),
            qualified_name: "mod::test_fn".into(),
            file: "mod.rs".into(),
            span: None,
            inputs: vec![],
            outputs: vec![],
            side_effects: vec![],
            dependencies: vec![],
            tests: vec![],
            docs: vec![],
            provenance: Provenance {
                extractor: "test".into(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        };
        graph.add_entity(entity);

        assert_eq!(graph.entity_count(), 1);
        assert_eq!(graph.edge_count(), 0);
        assert!(graph.find_entity(&"test-id".to_string()).is_some());
        assert!(graph.find_entity(&"nonexistent".to_string()).is_none());

        let edge = Edge {
            src: "test-id".into(),
            dst: "other-id".into(),
            kind: EdgeKind::Calls,
            provenance: Provenance {
                extractor: "test".into(),
                confidence: Confidence::Exact,
                evidence_refs: vec![],
            },
        };
        graph.add_edge(edge);
        assert_eq!(graph.edge_count(), 1);

        let outgoing = graph.outgoing_edges(&"test-id".to_string());
        assert_eq!(outgoing.len(), 1);

        let incoming = graph.incoming_edges(&"other-id".to_string());
        assert_eq!(incoming.len(), 1);
    }

    #[test]
    fn confidence_serde_round_trip() {
        let cases = vec![
            (r#""exact""#, Confidence::Exact),
            (r#""inferred""#, Confidence::Inferred),
            (r#""heuristic""#, Confidence::Heuristic),
        ];
        for (json, expected) in cases {
            let deser: Confidence = serde_json::from_str(json).expect("deserialize Confidence");
            assert_eq!(deser, expected);
            let ser = serde_json::to_string(&expected).expect("serialize Confidence");
            assert_eq!(ser, json);
        }
    }

    #[test]
    fn side_effect_serde_round_trip() {
        let cases = vec![
            (r#""fs.read""#, SideEffect::FsRead),
            (r#""net.http""#, SideEffect::NetHttp),
            (r#""process.exec""#, SideEffect::ProcessExec),
        ];
        for (json, expected) in cases {
            let deser: SideEffect = serde_json::from_str(json).expect("deserialize SideEffect");
            assert_eq!(deser, expected);
            let ser = serde_json::to_string(&expected).expect("serialize SideEffect");
            assert_eq!(ser, json);
        }
    }

    #[test]
    fn entity_kind_serde_round_trip() {
        let cases = vec![
            (r#""file""#, EntityKind::File),
            (r#""function""#, EntityKind::Function),
            (r#""trait""#, EntityKind::Trait),
        ];
        for (json, expected) in cases {
            let deser: EntityKind = serde_json::from_str(json).expect("deserialize EntityKind");
            assert_eq!(deser, expected);
            let ser = serde_json::to_string(&expected).expect("serialize EntityKind");
            assert_eq!(ser, json);
        }
    }

    #[test]
    fn type_hint_with_and_without_name() {
        // TypeHint for an output (no name)
        let json = r#"{"typeHint": "number"}"#;
        let hint: TypeHint = serde_json::from_str(json).expect("deserialize TypeHint");
        assert!(hint.name.is_none());
        assert_eq!(hint.type_hint, Some("number".to_string()));

        // TypeHint for an input (with name)
        let json = r#"{"name": "a", "typeHint": "number"}"#;
        let hint: TypeHint = serde_json::from_str(json).expect("deserialize TypeHint");
        assert_eq!(hint.name, Some("a".to_string()));
        assert_eq!(hint.type_hint, Some("number".to_string()));
    }
}
