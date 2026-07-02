use clap::ValueEnum;
use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, ValueEnum, schemars::JsonSchema)]
        #[serde(rename_all = "kebab-case")]
        pub enum $name {
            $($variant),+
        }

        impl $name {
            pub const fn as_str(self) -> &'static str {
                match self {
                    $(Self::$variant => $value),+
                }
            }
        }

        impl std::fmt::Display for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(self.as_str())
            }
        }

        impl std::str::FromStr for $name {
            type Err = String;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                match value {
                    $($value => Ok(Self::$variant),)+
                    _ => Err(format!("invalid {}: {value}", stringify!($name))),
                }
            }
        }
    };
}

string_enum!(EntityType {
    Scope => "scope",
    Goal => "goal",
    Roadmap => "roadmap",
    RoadmapSection => "roadmap-section",
    WorkPoint => "work-point",
    Plan => "plan",
    Todo => "todo",
    Issue => "issue",
    ReviewPoint => "review-point",
    Insight => "insight",
    ProjectRun => "project-run",
    GraphNode => "graph-node",
    GraphEdge => "graph-edge"
});

string_enum!(GoalStatus {
    Draft => "draft",
    Proposed => "proposed",
    Active => "active",
    Validated => "validated",
    Invalidated => "invalidated",
    Superseded => "superseded",
    Abandoned => "abandoned"
});

string_enum!(RoadmapStatus {
    Draft => "draft",
    Proposed => "proposed",
    Active => "active",
    Blocked => "blocked",
    Completed => "completed",
    Cancelled => "cancelled",
    Invalidated => "invalidated"
});

string_enum!(WorkPointStatus {
    Draft => "draft",
    Proposed => "proposed",
    Active => "active",
    Blocked => "blocked",
    Completed => "completed",
    Cancelled => "cancelled",
    Invalidated => "invalidated"
});

string_enum!(PlanStatus {
    Draft => "draft",
    Proposed => "proposed",
    Active => "active",
    Blocked => "blocked",
    Completed => "completed",
    Cancelled => "cancelled",
    Invalidated => "invalidated"
});

string_enum!(TodoStatus {
    Pending => "pending",
    InProgress => "in-progress",
    Blocked => "blocked",
    Completed => "completed",
    Cancelled => "cancelled"
});

string_enum!(IssueStatus {
    Open => "open",
    Blocked => "blocked",
    Resolved => "resolved",
    Reopened => "reopened"
});

string_enum!(ReviewPointStatus {
    Open => "open",
    Resolved => "resolved",
    AcceptedRisk => "accepted-risk"
});

string_enum!(Priority {
    Low => "low",
    Medium => "medium",
    High => "high",
    Urgent => "urgent"
});

string_enum!(EffortTier {
    Fast => "fast",
    Balanced => "balanced",
    Deep => "deep"
});

string_enum!(FileScopeSelectorType {
    Exact => "exact",
    Glob => "glob"
});

string_enum!(FileScopeIntent {
    Primary => "primary",
    Review => "review",
    Affected => "affected"
});

string_enum!(Severity {
    Low => "low",
    Medium => "medium",
    High => "high",
    Critical => "critical"
});

string_enum!(ValidationSeverity {
    Warning => "warning",
    Error => "error"
});

string_enum!(ValidationStatus {
    Valid => "valid",
    Warning => "warning",
    Invalid => "invalid"
});

string_enum!(ProjectionFormat {
    Markdown => "markdown",
    Json => "json"
});

string_enum!(InsightStatus {
    Active => "active",
    Superseded => "superseded",
    Archived => "archived"
});

string_enum!(InsightType {
    DesignDecision => "design-decision",
    EdgeCase => "edge-case",
    Optimization => "optimization",
    Constraint => "constraint",
    Assumption => "assumption",
    Risk => "risk",
    Context => "context"
});

string_enum!(ProjectRunStatus {
    Suggested => "suggested",
    Claimed => "claimed",
    Active => "active",
    Interrupted => "interrupted",
    Completed => "completed",
    Released => "released"
});

string_enum!(WorkPointKind {
    Feature => "feature",
    Corrective => "corrective",
    ReviewFix => "review-fix",
    ValidationFix => "validation-fix",
    FollowUp => "follow-up"
});

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WarningRecord {
    pub level: String,
    pub message: String,
    pub source: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunEvidence {
    pub implementation_run_refs: Vec<String>,
    pub warning_records: Vec<WarningRecord>,
    pub validation_finding_refs: Vec<String>,
    pub commit_sha: Option<String>,
    pub pr_url: Option<String>,
    pub linked_spec_ids: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunRecord {
    pub id: String,
    pub scope_key: String,
    pub goal_id: String,
    pub roadmap_id: String,
    pub work_point_id: String,
    pub repo_id: Option<String>,
    pub branch: Option<String>,
    pub worktree_id: Option<String>,
    pub session_id: Option<String>,
    pub run_id: Option<String>,
    pub profile_id: Option<String>,
    pub owner_id: String,
    pub idempotency_key: Option<String>,
    pub fencing_token: i64,
    pub lease_expires_at: String,
    pub heartbeat_at: String,
    pub status: ProjectRunStatus,
    pub evidence: ProjectRunEvidence,
    pub revision: i64,
    pub claimed_at: Option<String>,
    pub completed_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ProjectRunView {
    pub project_run: ProjectRunRecord,
    pub work_point: Option<WorkPointRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunnableWorkPointCandidate {
    pub work_point: WorkPointRecord,
    pub roadmap_id: String,
    pub roadmap_title: String,
    pub dependency_titles: Vec<String>,
    pub reasons: Vec<String>,
    pub required_reason: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockedCandidate {
    pub work_point_id: String,
    pub work_point_title: String,
    pub blocker_id: String,
    pub blocker_title: String,
    pub reason: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RunnableCandidates {
    pub roadmap_id: String,
    pub candidates: Vec<RunnableWorkPointCandidate>,
    pub blocked: Vec<BlockedCandidate>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkGraphNode {
    pub work_point: WorkPointRecord,
    pub plan_count: usize,
    pub has_active_lease: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkGraphEdge {
    pub source_id: String,
    pub target_id: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkGraph {
    pub nodes: Vec<WorkGraphNode>,
    pub edges: Vec<WorkGraphEdge>,
}

string_enum!(PlanningNodeKind {
    Goal => "goal",
    Roadmap => "roadmap",
    Milestone => "milestone",
    Work => "work",
    Plan => "plan",
    Task => "task",
    Run => "run",
    Acceptance => "acceptance",
    Evidence => "evidence",
    Issue => "issue",
    Review => "review",
    Insight => "insight"
});

string_enum!(PlanningEdgeKind {
    DecomposesTo => "decomposes-to",
    DependsOn => "depends-on",
    Blocks => "blocks",
    ParallelSafeWith => "parallel-safe-with",
    PlannedBy => "planned-by",
    ExecutedBy => "executed-by",
    Contains => "contains",
    Requires => "requires",
    Satisfies => "satisfies",
    EvidencedBy => "evidenced-by",
    Found => "found",
    AddressedBy => "addressed-by",
    Repairs => "repairs",
    Supersedes => "supersedes"
});

string_enum!(AcceptanceKind {
    Abstract => "abstract",
    Concrete => "concrete"
});

string_enum!(EvidenceKind {
    CommandResult => "command-result",
    TestResult => "test-result",
    ArtifactRef => "artifact-ref",
    CommitRef => "commit-ref",
    PrRef => "pr-ref",
    Review => "review",
    TraceExcerpt => "trace-excerpt",
    ExternalUrl => "external-url"
});

/// Acceptance node payload shape (stored in payload_json column).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptancePayload {
    #[serde(rename = "acceptanceKind")]
    pub acceptance_kind: String,
    #[serde(default)]
    pub description: String,
    #[serde(default, rename = "verificationPolicy")]
    pub verification_policy: String,
    #[serde(default, rename = "requiredEvidenceKinds")]
    pub required_evidence_kinds: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub waiver: Option<String>,
}

/// Evidence node payload shape (stored in payload_json column).
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvidencePayload {
    #[serde(rename = "evidenceKind")]
    pub evidence_kind: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub reference: String,
    #[serde(default)]
    pub content: String,
    #[serde(default, rename = "capturedAt")]
    pub captured_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningGraphNode {
    pub id: String,
    pub scope_key: String,
    pub kind: PlanningNodeKind,
    pub title: String,
    pub summary: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningGraphEdge {
    pub id: String,
    pub scope_key: String,
    pub kind: PlanningEdgeKind,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: String,
    pub payload: serde_json::Value,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphNodeView {
    pub node: PlanningGraphNode,
    pub incoming_edges: Vec<PlanningGraphEdge>,
    pub outgoing_edges: Vec<PlanningGraphEdge>,
    pub connected_nodes: Vec<serde_json::Value>,
    pub tags: Vec<String>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphEdgeView {
    pub edge: PlanningGraphEdge,
    pub source_node: serde_json::Value,
    pub target_node: serde_json::Value,
    pub validation: ValidationReport,
}

/// Acceptance view: node + linked requirements, coverage, and evidence paths.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AcceptanceView {
    pub node: PlanningGraphNode,
    /// Nodes that require this acceptance (via Requires edge).
    pub required_by: Vec<PlanningGraphNode>,
    /// Abstract acceptances this concrete satisfies (via Satisfies edge from self).
    pub satisfied_abstracts: Vec<PlanningGraphNode>,
    /// Concrete acceptances that satisfy this abstract (via Satisfies edge to self).
    pub satisfying_concretes: Vec<PlanningGraphNode>,
    /// Evidence attached to this acceptance (via EvidencedBy edge).
    pub attached_evidence: Vec<PlanningGraphNode>,
    pub validation: ValidationReport,
}

/// Evidence view: node + linked targets.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceView {
    pub node: PlanningGraphNode,
    /// Targets this evidence is attached to (via EvidencedBy edge).
    pub attached_to: Vec<PlanningGraphNode>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ScopeRecord {
    pub scope_key: String,
    pub scope_type: Option<String>,
    pub parent_scope_key: Option<String>,
    pub metadata: serde_json::Value,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoalRecord {
    pub id: String,
    pub scope_key: String,
    pub correlation_id: String,
    pub title: String,
    pub description: String,
    pub acceptance_criteria: Vec<String>,
    pub rejection_criteria: Vec<String>,
    pub status: GoalStatus,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapRecord {
    pub id: String,
    pub scope_key: String,
    pub goal_id: String,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub status: RoadmapStatus,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapSectionRecord {
    pub id: String,
    pub scope_key: String,
    pub roadmap_id: String,
    pub slug: String,
    pub title: String,
    pub summary: String,
    pub ordering: i64,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkPointRecord {
    pub id: String,
    pub scope_key: String,
    pub roadmap_id: String,
    pub section_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub status: WorkPointStatus,
    pub ordering: i64,
    pub dependency_ids: Vec<String>,
    pub validation_expectations: Vec<String>,
    pub effort_tier: EffortTier,
    pub kind: WorkPointKind,
    pub priority: Priority,
    pub repairs_work_point_ids: Vec<String>,
    pub supersedes_work_point_ids: Vec<String>,
    pub blocks_work_point_ids: Vec<String>,
    pub file_scopes: Vec<FileScopeRecord>,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanRecord {
    pub id: String,
    pub scope_key: String,
    pub goal_id: String,
    pub roadmap_id: String,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub scope: String,
    pub assumptions: Vec<String>,
    pub stop_conditions: Vec<String>,
    pub validation_steps: Vec<String>,
    pub targeted_work_point_ids: Vec<String>,
    pub effort_tier: EffortTier,
    pub routing_hint: Option<String>,
    pub allow_parallel_overlap: bool,
    pub file_scopes: Vec<FileScopeRecord>,
    pub status: PlanStatus,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TodoRecord {
    pub id: String,
    pub scope_key: String,
    pub plan_id: Option<String>,
    pub work_point_id: Option<String>,
    pub title: String,
    pub summary: String,
    pub status: TodoStatus,
    pub priority: Priority,
    pub effort_tier: EffortTier,
    pub file_scopes: Vec<FileScopeRecord>,
    pub evidence_refs: Vec<String>,
    pub tags: Vec<String>,
    pub ordering: i64,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileScopeRecord {
    pub selector_type: FileScopeSelectorType,
    pub selector: String,
    pub intent: FileScopeIntent,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueRecord {
    pub id: String,
    pub scope_key: String,
    pub correlation_id: String,
    pub title: String,
    pub summary: String,
    pub status: IssueStatus,
    pub severity: Severity,
    pub related_entity_type: Option<EntityType>,
    pub related_entity_id: Option<String>,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReviewPointRecord {
    pub id: String,
    pub scope_key: String,
    pub attached_entity_type: EntityType,
    pub attached_entity_id: String,
    pub title: String,
    pub summary: String,
    pub status: ReviewPointStatus,
    pub severity: Severity,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InsightRecord {
    pub id: String,
    pub scope_key: String,
    pub correlation_id: String,
    pub title: String,
    pub content: String,
    pub insight_type: InsightType,
    pub parent_entity_type: EntityType,
    pub parent_entity_id: String,
    pub tags: Vec<String>,
    pub status: InsightStatus,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InsightView {
    pub insight: InsightRecord,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TagInfo {
    pub tag: String,
    pub entity_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TokenEstimate {
    pub entity_tokens: usize,
    pub related_tokens: usize,
    pub insight_tokens: usize,
    pub total_tokens: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityContextBundle {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub entity: serde_json::Value,
    pub parent_summary: Option<serde_json::Value>,
    pub children: Vec<serde_json::Value>,
    pub insights: Vec<InsightRecord>,
    pub related_insights: Vec<InsightRecord>,
    pub tags: Vec<String>,
    pub validation: ValidationReport,
    pub token_estimate: TokenEstimate,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionContextBundle {
    pub session_id: Option<String>,
    pub correlation_id: Option<String>,
    pub entities_touched: Vec<SearchResult>,
    pub insights_recorded: Vec<InsightRecord>,
    pub validation_summary: SessionValidationSummary,
    pub token_estimate: TokenEstimate,
    pub active_project_runs: Vec<ProjectRunRecord>,
    pub active_work_points: Vec<WorkPointRecord>,
    pub active_plans: Vec<PlanRecord>,
    pub next_pending_todos: Vec<TodoRecord>,
    pub open_blocking_issues: Vec<IssueRecord>,
    pub open_blocking_review_points: Vec<ReviewPointRecord>,
    pub recommended_next_action: Option<String>,
    pub context_warnings: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionValidationSummary {
    pub error_count: usize,
    pub warning_count: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningEvent {
    pub event_id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub aggregate_type: EntityType,
    pub aggregate_id: String,
    pub correlation_id: String,
    pub causation_id: Option<String>,
    pub run_id: String,
    pub stream_id: String,
    pub sequence: u64,
    pub parent_event_id: Option<String>,
    pub event_type: String,
    pub timestamp: String,
    pub payload: serde_json::Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFinding {
    pub finding_id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub severity: ValidationSeverity,
    pub code: String,
    pub message: String,
    pub scope_key: String,
    pub fingerprint: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub status: ValidationStatus,
    pub findings: Vec<ValidationFinding>,
}

impl ValidationReport {
    pub fn from_findings(mut findings: Vec<ValidationFinding>) -> Self {
        findings.sort_by(|left, right| {
            left.severity
                .as_str()
                .cmp(right.severity.as_str())
                .then_with(|| left.code.cmp(&right.code))
                .then_with(|| left.message.cmp(&right.message))
        });

        let status = if findings
            .iter()
            .any(|finding| finding.severity == ValidationSeverity::Error)
        {
            ValidationStatus::Invalid
        } else if findings.is_empty() {
            ValidationStatus::Valid
        } else {
            ValidationStatus::Warning
        };

        Self { status, findings }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[schemars(bound = "T: schemars::JsonSchema")]
#[serde(rename_all = "camelCase")]
pub struct MutationResult<T>
where
    T: Serialize,
{
    pub record: T,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GoalView {
    pub goal: GoalRecord,
    pub roadmaps: Vec<RoadmapRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapView {
    pub roadmap: RoadmapRecord,
    pub sections: Vec<RoadmapSectionRecord>,
    pub work_points: Vec<WorkPointRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkPointView {
    pub work_point: WorkPointRecord,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub plan: PlanRecord,
    pub todos: Vec<TodoRecord>,
    pub review_points: Vec<ReviewPointRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IssueView {
    pub issue: IssueRecord,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct EntityValidationView {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ValidationRunReport {
    pub status: ValidationStatus,
    pub scope_mode: String,
    pub scope_key: String,
    pub findings: Vec<ValidationFinding>,
    pub entity_reports: Vec<EntityValidationView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningHealthReport {
    pub db_path: String,
    pub schema_version: String,
    pub event_count: i64,
    pub active_validation_finding_count: i64,
    pub scope_count: i64,
    pub goal_count: i64,
    pub roadmap_count: i64,
    pub roadmap_section_count: i64,
    pub work_point_count: i64,
    pub plan_count: i64,
    pub todo_count: i64,
    pub issue_count: i64,
    pub review_point_count: i64,
    pub insight_count: i64,
    pub project_run_count: i64,
    pub graph_node_count: i64,
    pub graph_edge_count: i64,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RenderedProjection {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub format: ProjectionFormat,
    pub revision: i64,
    pub output_path: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SearchResult {
    pub entity_type: String,
    pub id: String,
    pub title: String,
    pub status: String,
    pub updated_at: String,
    pub created_at: String,
}

/// A registered worktree tracked by the planning system.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorktreeRecord {
    pub id: String,
    pub scope_key: String,
    pub repo_uri: Option<String>,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
    pub project_run_id: Option<String>,
    pub session_id: Option<String>,
    pub status: WorktreeStatus,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum WorktreeStatus {
    Active,
    Archived,
    CleanupIntent,
}

impl std::fmt::Display for WorktreeStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            WorktreeStatus::Active => write!(f, "active"),
            WorktreeStatus::Archived => write!(f, "archived"),
            WorktreeStatus::CleanupIntent => write!(f, "cleanup-intent"),
        }
    }
}

/// Input for attaching/registering a worktree.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachWorktreeInput {
    pub id: Option<String>,
    pub scope_key: Option<String>,
    pub repo_uri: Option<String>,
    pub branch: Option<String>,
    pub worktree_path: Option<String>,
    pub project_run_id: Option<String>,
    pub session_id: Option<String>,
    pub correlation_id: Option<String>,
}

/// Session summary for listing.
#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SessionSummary {
    pub session_id: String,
    pub scope: String,
    pub created_at: Option<String>,
    pub last_seen: Option<String>,
    pub event_count: i64,
    pub active_project_runs: i64,
}

/// Helper to parse worktree status from string (lenient — defaults to Active).
pub fn parse_worktree_status(s: &str) -> WorktreeStatus {
    match s {
        "archived" => WorktreeStatus::Archived,
        "cleanup-intent" => WorktreeStatus::CleanupIntent,
        _ => WorktreeStatus::Active,
    }
}

/// Strict worktree status parser for storage row deserialisation.
pub fn parse_worktree_status_strict(s: &str) -> Result<WorktreeStatus, String> {
    match s {
        "active" => Ok(WorktreeStatus::Active),
        "archived" => Ok(WorktreeStatus::Archived),
        "cleanup-intent" => Ok(WorktreeStatus::CleanupIntent),
        other => Err(format!("invalid worktree status: {other}")),
    }
}

// ─── Manifest Types ────────────────────────────────────────────────────────

/// A planning manifest: a complete graph expressed as nodes + edges.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    /// Manifest schema version (planning-manifest/v1).
    #[serde(default = "default_manifest_schema_version")]
    pub schema_version: String,
    /// Scope key all entities belong to.
    pub scope: String,
    /// Nodes to create or update.
    #[serde(default)]
    pub nodes: Vec<ManifestNode>,
    /// Edges to create or update.
    #[serde(default)]
    pub edges: Vec<ManifestEdge>,
}

fn default_manifest_schema_version() -> String {
    "planning-manifest/v1".to_string()
}

/// A node definition in a manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestNode {
    /// Stable user-provided ID. UUID generated if omitted.
    #[serde(default)]
    pub id: Option<String>,
    pub kind: PlanningNodeKind,
    pub title: String,
    pub summary: String,
    /// Lifecycle status (e.g. "active", "draft", "completed").
    pub status: String,
    /// Kind-specific payload JSON object.
    #[serde(default)]
    pub payload: serde_json::Value,
    #[serde(default)]
    pub tags: Vec<String>,

    // ── Shorthand edge fields (expanded during parsing) ──
    /// Shorthand: creates `depends-on` edges from this node to each listed ID.
    #[serde(default)]
    pub depends_on: Vec<String>,
    /// Shorthand: creates `blocks` edges from this node to each listed ID.
    #[serde(default)]
    pub blocks: Vec<String>,
    /// Shorthand: creates `decomposes-to` edges from this node to each listed ID.
    #[serde(default)]
    pub decomposes_to: Vec<String>,
    /// Shorthand: creates `planned-by` edges from this node to each listed ID.
    #[serde(default)]
    pub planned_by: Vec<String>,
    /// Shorthand (on plan nodes): creates `planned-by` edges from each listed work ID to this plan.
    #[serde(default)]
    pub targeted_work: Vec<String>,
    /// Shorthand: creates `repairs` edges from this node to each listed ID.
    #[serde(default)]
    pub repairs: Vec<String>,
    /// Shorthand: creates `supersedes` edges from this node to each listed ID.
    #[serde(default)]
    pub supersedes: Vec<String>,

    // ── Acceptance-specific fields ──
    #[serde(default)]
    pub acceptance_kind: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub verification_policy: Option<String>,
    #[serde(default)]
    pub required_evidence_kinds: Vec<String>,
    #[serde(default)]
    pub waiver: Option<String>,

    // ── Evidence-specific fields ──
    #[serde(default)]
    pub evidence_kind: Option<String>,
    #[serde(default)]
    pub reference: Option<String>,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub captured_at: Option<String>,
}

/// An edge definition in a manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestEdge {
    /// Stable user-provided ID. UUID generated if omitted.
    #[serde(default)]
    pub id: Option<String>,
    pub kind: PlanningEdgeKind,
    pub source_node_id: String,
    pub target_node_id: String,
    #[serde(default = "default_edge_status")]
    pub status: String,
    #[serde(default)]
    pub payload: serde_json::Value,
}

fn default_edge_status() -> String {
    "active".to_string()
}

/// Result of a manifest apply or diff operation.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestApplyResult {
    pub created_nodes: Vec<String>,
    pub revised_nodes: Vec<String>,
    pub unchanged_nodes: Vec<String>,
    pub created_edges: Vec<String>,
    pub revised_edges: Vec<String>,
    pub unchanged_edges: Vec<String>,
    pub conflicts: Vec<ManifestConflict>,
    pub validation: Option<ValidationReport>,
    pub total_nodes: usize,
    pub total_edges: usize,
}

/// A conflict detected during manifest application.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestConflict {
    pub entity_type: String,
    pub entity_id: String,
    pub reason: String,
}

/// Result of diffing a manifest against the database.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDiffResult {
    pub added_nodes: Vec<String>,
    pub removed_nodes: Vec<String>,
    pub changed_nodes: Vec<ManifestDiffEntry>,
    pub unchanged_nodes: Vec<String>,
    pub added_edges: Vec<String>,
    pub removed_edges: Vec<String>,
    pub changed_edges: Vec<ManifestDiffEntry>,
    pub unchanged_edges: Vec<String>,
}

/// A single diff entry for a changed entity.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ManifestDiffEntry {
    pub entity_id: String,
    pub diffs: Vec<FieldDiff>,
}

/// A field-level difference.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FieldDiff {
    pub field: String,
    pub manifest_value: serde_json::Value,
    pub db_value: serde_json::Value,
}

/// Compact form of a graph node for concise output.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompactGraphNode {
    pub id: String,
    pub kind: PlanningNodeKind,
    pub title: String,
    pub status: String,
}

/// Compact form of a graph edge for concise output.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct CompactGraphEdge {
    pub id: String,
    pub kind: PlanningEdgeKind,
    pub source_node_id: String,
    pub target_node_id: String,
    pub status: String,
}

// ─── Graph Runnable Types ──────────────────────────────────────────────────

/// A candidate work node that is runnable in the graph.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphRunnableCandidate {
    pub node_id: String,
    pub title: String,
    pub status: String,
    /// Why this candidate is runnable: "ready", "urgent_fix", "resolves_blocker".
    pub reason: String,
    pub incomplete_dependencies: Vec<String>,
    pub active_blockers: Vec<String>,
}

/// A work node that is blocked.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockedGraphCandidate {
    pub node_id: String,
    pub title: String,
    pub reason: String,
    pub blocker_ids: Vec<String>,
}

/// Result of a graph runnable query.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct GraphRunnableResult {
    pub candidates: Vec<GraphRunnableCandidate>,
    pub blocked: Vec<BlockedGraphCandidate>,
}

// ─── Bulk Transition Types ─────────────────────────────────────────────────

/// Input for bulk status transitions on graph nodes.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BulkTransitionInput {
    pub scope_key: String,
    pub node_ids: Option<Vec<String>>,
    pub filter: Option<String>,
    pub status: String,
    pub correlation_id: String,
    pub run_id: Option<String>,
}

/// A single rejection in a bulk transition.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkTransitionRejection {
    pub node_id: String,
    pub reason: String,
}

/// Result of a bulk transition.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BulkTransitionResult {
    pub transitioned: Vec<String>,
    pub rejected: Vec<BulkTransitionRejection>,
    pub total_matched: usize,
    pub total_transitioned: usize,
}

// ─── Intent Types ───────────────────────────────────────────────────────────

/// A planning intent document — lighter than a full manifest.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PlanningIntent {
    #[serde(default = "default_intent_schema_version")]
    pub schema_version: String,
    pub scope: String,
    pub intent: String,
    #[serde(default)]
    pub constraints: Vec<String>,
    #[serde(default)]
    pub non_goals: Vec<String>,
    #[serde(default)]
    pub dependencies: Vec<IntentDependency>,
    #[serde(default)]
    pub deliverables: Vec<String>,
    #[serde(default)]
    pub verification: Vec<String>,
}

fn default_intent_schema_version() -> String {
    "planning-intent/v1".to_string()
}

/// A dependency in an intent document.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct IntentDependency {
    pub kind: String,
    pub description: String,
}
