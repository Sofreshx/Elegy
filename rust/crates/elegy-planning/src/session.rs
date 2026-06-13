use std::{env, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PlanningStoreError;

const SESSION_FILE_NAME: &str = "planning-session.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveProjectRunState {
    pub project_run_id: String,
    pub goal_id: String,
    pub roadmap_id: String,
    pub work_point_id: String,
    pub status: String,
    pub claimed_at: String,
    pub activated_at: Option<String>,
    pub evidence_refs: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningSession {
    pub session_id: String,
    pub scope: String,
    pub created_at: String,
    pub last_used: String,
    pub active_project_run: Option<ActiveProjectRunState>,
    pub last_active_work_point_id: Option<String>,
    pub last_completed_work_point_id: Option<String>,
}

pub fn session_file_path() -> PathBuf {
    let home = env::var_os("HOME")
        .or_else(|| env::var_os("USERPROFILE"))
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."));
    home.join(".elegy").join(SESSION_FILE_NAME)
}

pub fn init_session(scope: &str) -> Result<PlanningSession, PlanningStoreError> {
    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| PlanningStoreError::TimeFormat)?;

    let session = PlanningSession {
        session_id: Uuid::new_v4().to_string(),
        scope: scope.to_string(),
        created_at: now.clone(),
        last_used: now,
        active_project_run: None,
        last_active_work_point_id: None,
        last_completed_work_point_id: None,
    };

    write_session(&session)?;
    Ok(session)
}

pub fn use_session(session_id: &str) -> Result<PlanningSession, PlanningStoreError> {
    let mut session = read_session()?.ok_or_else(|| {
        PlanningStoreError::InvalidInput(
            "no active session; run `elegy-planning session init` first".to_string(),
        )
    })?;

    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| PlanningStoreError::TimeFormat)?;

    session.session_id = session_id.to_string();
    session.last_used = now;

    write_session(&session)?;
    Ok(session)
}

pub fn show_session() -> Result<Option<PlanningSession>, PlanningStoreError> {
    read_session()
}

pub fn resolve_session_correlation_id() -> Result<Option<String>, PlanningStoreError> {
    Ok(read_session()?.map(|s| s.session_id))
}

pub fn update_session_file(
    session_id: &str,
    scope: &str,
) -> Result<PlanningSession, PlanningStoreError> {
    let mut session = read_session()?.unwrap_or(PlanningSession {
        session_id: uuid::Uuid::new_v4().to_string(),
        scope: scope.to_string(),
        created_at: String::new(),
        last_used: String::new(),
        active_project_run: None,
        last_active_work_point_id: None,
        last_completed_work_point_id: None,
    });

    let now = time::OffsetDateTime::now_utc()
        .format(&time::format_description::well_known::Rfc3339)
        .map_err(|_| PlanningStoreError::TimeFormat)?;

    session.session_id = session_id.to_string();
    session.scope = scope.to_string();
    session.last_used = now.clone();
    if session.created_at.is_empty() {
        session.created_at = now;
    }

    write_session(&session)?;
    Ok(session)
}

pub fn read_session_file() -> Result<Option<PlanningSession>, PlanningStoreError> {
    read_session()
}

fn read_session() -> Result<Option<PlanningSession>, PlanningStoreError> {
    let path = session_file_path();
    if !path.exists() {
        return Ok(None);
    }
    let content =
        fs::read_to_string(&path).map_err(|source| PlanningStoreError::CreateDirectory {
            path: path.clone(),
            source,
        })?;
    let session: PlanningSession = serde_json::from_str(&content)?;
    Ok(Some(session))
}

fn write_session(session: &PlanningSession) -> Result<(), PlanningStoreError> {
    let path = session_file_path();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PlanningStoreError::CreateDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let content = serde_json::to_string_pretty(session)?;
    let tmp_path = path.with_extension("json.tmp");
    fs::write(&tmp_path, &content).map_err(|source| PlanningStoreError::CreateDirectory {
        path: tmp_path.clone(),
        source,
    })?;
    fs::rename(&tmp_path, &path)
        .map_err(|source| PlanningStoreError::CreateDirectory { path, source })?;
    Ok(())
}

pub fn set_active_project_run(state: ActiveProjectRunState) -> Result<(), PlanningStoreError> {
    let mut session = match read_session()? {
        Some(s) => s,
        None => PlanningSession {
            session_id: uuid::Uuid::new_v4().to_string(),
            scope: "default".to_string(),
            created_at: String::new(),
            last_used: String::new(),
            active_project_run: None,
            last_active_work_point_id: None,
            last_completed_work_point_id: None,
        },
    };
    session.active_project_run = Some(state.clone());
    session.last_active_work_point_id = Some(state.work_point_id.clone());
    write_session(&session)?;
    Ok(())
}

pub fn clear_active_project_run(
    last_completed_work_point_id: Option<String>,
) -> Result<(), PlanningStoreError> {
    let mut session = match read_session()? {
        Some(s) => s,
        None => return Ok(()),
    };
    session.active_project_run = None;
    if let Some(id) = last_completed_work_point_id {
        session.last_completed_work_point_id = Some(id);
    }
    write_session(&session)?;
    Ok(())
}
