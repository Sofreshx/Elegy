use std::{env, fs, path::PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::PlanningStoreError;

const SESSION_FILE_NAME: &str = "planning-session.json";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanningSession {
    pub session_id: String,
    pub scope: String,
    pub created_at: String,
    pub last_used: String,
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
    fs::write(&path, content)
        .map_err(|source| PlanningStoreError::CreateDirectory { path, source })?;
    Ok(())
}
