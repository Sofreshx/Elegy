use clap::ValueEnum;
use serde::{Deserialize, Serialize};

macro_rules! string_enum {
    ($name:ident { $($variant:ident => $value:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, Serialize, Deserialize, PartialEq, Eq, ValueEnum)]
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
    ReviewPoint => "review-point"
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    pub status: PlanStatus,
    pub tags: Vec<String>,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
    pub evidence_refs: Vec<String>,
    pub tags: Vec<String>,
    pub ordering: i64,
    pub revision: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationFinding {
    pub finding_id: String,
    pub entity_type: EntityType,
    pub entity_id: String,
    pub severity: ValidationSeverity,
    pub code: String,
    pub message: String,
    pub created_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct MutationResult<T>
where
    T: Serialize,
{
    pub record: T,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct GoalView {
    pub goal: GoalRecord,
    pub roadmaps: Vec<RoadmapRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RoadmapView {
    pub roadmap: RoadmapRecord,
    pub sections: Vec<RoadmapSectionRecord>,
    pub work_points: Vec<WorkPointRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct WorkPointView {
    pub work_point: WorkPointRecord,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlanView {
    pub plan: PlanRecord,
    pub todos: Vec<TodoRecord>,
    pub review_points: Vec<ReviewPointRecord>,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct IssueView {
    pub issue: IssueRecord,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EntityValidationView {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub validation: ValidationReport,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ValidationRunReport {
    pub status: ValidationStatus,
    pub findings: Vec<ValidationFinding>,
    pub entity_reports: Vec<EntityValidationView>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
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
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RenderedProjection {
    pub entity_type: EntityType,
    pub entity_id: String,
    pub format: ProjectionFormat,
    pub revision: i64,
    pub output_path: String,
}
