use async_trait::async_trait;
use elegy_contracts::{
    validate_agent_event_envelope, validate_agent_request_envelope,
    validate_agent_response_envelope, AgentEventEnvelope, AgentEventPayload,
    AgentEventSource, AgentEventType, AgentRequestEnvelope, AgentResponseEnvelope,
    AgentResponseStatus,
};
use std::collections::{BTreeMap, VecDeque};
use std::sync::{Arc, Mutex};
use thiserror::Error;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use tokio::sync::mpsc;

#[derive(Clone, Debug)]
pub struct BrokerOptions {
    pub replay_capacity: usize,
    pub subscriber_queue_capacity: usize,
}

impl Default for BrokerOptions {
    fn default() -> Self {
        Self {
            replay_capacity: 256,
            subscriber_queue_capacity: 64,
        }
    }
}

#[derive(Clone, Debug)]
pub struct SubscriptionOptions {
    pub after_sequence: Option<u64>,
    pub include_replay: bool,
}

impl Default for SubscriptionOptions {
    fn default() -> Self {
        Self {
            after_sequence: None,
            include_replay: true,
        }
    }
}

#[derive(Debug)]
pub struct AgentEventSubscription {
    pub replay: Vec<AgentEventEnvelope>,
    pub receiver: mpsc::Receiver<AgentEventEnvelope>,
}

#[derive(Clone, Debug)]
pub struct AgentEventBroker {
    inner: Arc<BrokerInner>,
}

#[derive(Clone, Debug)]
pub struct AgentRunHandle {
    broker: AgentEventBroker,
    run_id: String,
    request_id: String,
    stream_id: String,
}

impl AgentRunHandle {
    pub fn run_id(&self) -> &str {
        self.run_id.as_str()
    }

    pub fn request_id(&self) -> &str {
        self.request_id.as_str()
    }

    pub fn stream_id(&self) -> &str {
        self.stream_id.as_str()
    }

    pub fn publish(
        &self,
        event_type: AgentEventType,
        source: AgentEventSource,
        payload: AgentEventPayload,
        ephemeral: bool,
        parent_event_id: Option<String>,
    ) -> Result<AgentEventEnvelope, AgentBrokerError> {
        self.broker.publish_internal(
            self.run_id.as_str(),
            event_type,
            source,
            payload,
            ephemeral,
            parent_event_id,
            None,
        )
    }

    pub fn complete(
        &self,
        mut response: AgentResponseEnvelope,
    ) -> Result<AgentResponseEnvelope, AgentBrokerError> {
        if response.request_id.trim().is_empty() {
            response.request_id = self.request_id.clone();
        }

        if response.run_id.trim().is_empty() {
            response.run_id = self.run_id.clone();
        }

        let validation = validate_agent_response_envelope(&response);
        if !validation.is_valid() {
            return Err(AgentBrokerError::Validation(validation.issues));
        }

        let event_type = match response.status {
            AgentResponseStatus::Completed => AgentEventType::RunCompleted,
            AgentResponseStatus::Failed => AgentEventType::RunFailed,
            AgentResponseStatus::Cancelled => AgentEventType::RunCancelled,
        };

        let payload = AgentEventPayload {
            content: response.messages.last().map(|message| message.content.clone()),
            error_code: response.error_code.clone(),
            error_message: response.error_message.clone(),
            usage: Some(response.usage.clone()),
            metadata: if response.metadata.is_empty() {
                None
            } else {
                Some(response.metadata.clone())
            },
            ..AgentEventPayload::default()
        };

        self.broker.publish_internal(
            self.run_id.as_str(),
            event_type,
            AgentEventSource::Broker,
            payload,
            false,
            None,
            Some(response.clone()),
        )?;

        Ok(response)
    }

    pub fn fail(
        &self,
        error_code: impl Into<String>,
        error_message: impl Into<String>,
    ) -> Result<AgentResponseEnvelope, AgentBrokerError> {
        self.complete(AgentResponseEnvelope {
            request_id: self.request_id.clone(),
            run_id: self.run_id.clone(),
            status: AgentResponseStatus::Failed,
            error_code: Some(error_code.into()),
            error_message: Some(error_message.into()),
            ..AgentResponseEnvelope::default()
        })
    }
}

#[async_trait]
pub trait AgentRequestAdapter: Send + Sync {
    async fn execute(
        &self,
        request: AgentRequestEnvelope,
        run: AgentRunHandle,
    ) -> Result<AgentResponseEnvelope, AgentAdapterError>;
}

#[derive(Clone, Debug, PartialEq, Eq, Error)]
#[error("{code}: {message}")]
pub struct AgentAdapterError {
    pub code: String,
    pub message: String,
}

impl AgentAdapterError {
    pub fn new(code: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            message: message.into(),
        }
    }
}

#[derive(Debug, Error)]
pub enum AgentBrokerError {
    #[error("agent envelope validation failed: {0:?}")]
    Validation(Vec<String>),
    #[error("agent run '{0}' already exists")]
    DuplicateRun(String),
    #[error("agent run '{0}' was not found")]
    UnknownRun(String),
    #[error("agent run '{0}' has already completed")]
    RunCompleted(String),
    #[error("broker state is unavailable")]
    Poisoned,
    #[error("failed to format broker timestamp: {0}")]
    Timestamp(String),
    #[error("adapter execution failed: {0}: {1}")]
    AdapterFailed(String, String),
}

impl AgentEventBroker {
    pub fn new(options: BrokerOptions) -> Self {
        Self {
            inner: Arc::new(BrokerInner {
                options,
                state: Mutex::new(BrokerState::default()),
            }),
        }
    }

    pub fn start_request(
        &self,
        request: AgentRequestEnvelope,
    ) -> Result<AgentRunHandle, AgentBrokerError> {
        let validation = validate_agent_request_envelope(&request);
        if !validation.is_valid() {
            return Err(AgentBrokerError::Validation(validation.issues));
        }

        let run_id = request.request_id.clone();
        let stream_id = request
            .context
            .session_id
            .clone()
            .or(request.context.conversation_id.clone())
            .unwrap_or_else(|| request.request_id.clone());
        let request_id = request.request_id.clone();

        {
            let mut state = self
                .inner
                .state
                .lock()
                .map_err(|_| AgentBrokerError::Poisoned)?;

            if state.runs.contains_key(run_id.as_str()) {
                return Err(AgentBrokerError::DuplicateRun(run_id));
            }

            state.runs.insert(
                request_id.clone(),
                RunState {
                    stream_id: stream_id.clone(),
                    next_sequence: 1,
                    ..RunState::default()
                },
            );
        }

        let handle = AgentRunHandle {
            broker: self.clone(),
            run_id: request_id.clone(),
            request_id,
            stream_id,
        };

        handle.publish(
            AgentEventType::RequestAccepted,
            AgentEventSource::Broker,
            AgentEventPayload::default(),
            false,
            None,
        )?;
        handle.publish(
            AgentEventType::RunStarted,
            AgentEventSource::Broker,
            AgentEventPayload::default(),
            false,
            None,
        )?;

        Ok(handle)
    }

    pub fn subscribe(
        &self,
        run_id: &str,
        options: SubscriptionOptions,
    ) -> Result<AgentEventSubscription, AgentBrokerError> {
        let (sender, receiver) = mpsc::channel(self.inner.options.subscriber_queue_capacity);
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| AgentBrokerError::Poisoned)?;
        let subscriber_id = state.next_subscriber_id;
        state.next_subscriber_id += 1;

        let run = state
            .runs
            .get_mut(run_id)
            .ok_or_else(|| AgentBrokerError::UnknownRun(run_id.to_string()))?;

        let replay = if options.include_replay {
            run.events
                .iter()
                .filter(|event| {
                    options
                        .after_sequence
                        .is_none_or(|after_sequence| event.sequence > after_sequence)
                })
                .cloned()
                .collect()
        } else {
            Vec::new()
        };

        run.subscribers.insert(subscriber_id, sender);

        Ok(AgentEventSubscription { replay, receiver })
    }

    pub fn events_for_run(&self, run_id: &str) -> Result<Vec<AgentEventEnvelope>, AgentBrokerError> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| AgentBrokerError::Poisoned)?;
        let run = state
            .runs
            .get(run_id)
            .ok_or_else(|| AgentBrokerError::UnknownRun(run_id.to_string()))?;

        Ok(run.events.iter().cloned().collect())
    }

    pub fn response_for_run(
        &self,
        run_id: &str,
    ) -> Result<Option<AgentResponseEnvelope>, AgentBrokerError> {
        let state = self
            .inner
            .state
            .lock()
            .map_err(|_| AgentBrokerError::Poisoned)?;
        let run = state
            .runs
            .get(run_id)
            .ok_or_else(|| AgentBrokerError::UnknownRun(run_id.to_string()))?;

        Ok(run.response.clone())
    }

    pub async fn process_request<A>(
        &self,
        request: AgentRequestEnvelope,
        adapter: &A,
    ) -> Result<AgentResponseEnvelope, AgentBrokerError>
    where
        A: AgentRequestAdapter,
    {
        let run = self.start_request(request.clone())?;
        match adapter.execute(request, run.clone()).await {
            Ok(response) => run.complete(response),
            Err(error) => {
                let _ = run.fail(error.code.clone(), error.message.clone());
                Err(AgentBrokerError::AdapterFailed(error.code, error.message))
            }
        }
    }

    fn publish_internal(
        &self,
        run_id: &str,
        event_type: AgentEventType,
        source: AgentEventSource,
        payload: AgentEventPayload,
        ephemeral: bool,
        parent_event_id: Option<String>,
        terminal_response: Option<AgentResponseEnvelope>,
    ) -> Result<AgentEventEnvelope, AgentBrokerError> {
        let mut state = self
            .inner
            .state
            .lock()
            .map_err(|_| AgentBrokerError::Poisoned)?;
        let run = state
            .runs
            .get_mut(run_id)
            .ok_or_else(|| AgentBrokerError::UnknownRun(run_id.to_string()))?;

        if run.terminal {
            return Err(AgentBrokerError::RunCompleted(run_id.to_string()));
        }

        let sequence = run.next_sequence;
        run.next_sequence += 1;

        let event = AgentEventEnvelope {
            event_id: format!("{run_id}:{sequence}"),
            run_id: run_id.to_string(),
            stream_id: run.stream_id.clone(),
            sequence,
            parent_event_id,
            timestamp: format_timestamp()?,
            ephemeral,
            event_type,
            source,
            payload,
        };

        let validation = validate_agent_event_envelope(&event);
        if !validation.is_valid() {
            return Err(AgentBrokerError::Validation(validation.issues));
        }

        if !event.ephemeral {
            run.events.push_back(event.clone());
            while run.events.len() > self.inner.options.replay_capacity {
                run.events.pop_front();
            }
        }

        let mut stale_subscribers = Vec::new();
        for (subscriber_id, subscriber) in &run.subscribers {
            if subscriber.try_send(event.clone()).is_err() {
                stale_subscribers.push(*subscriber_id);
            }
        }

        for subscriber_id in stale_subscribers {
            run.subscribers.remove(&subscriber_id);
        }

        if let Some(response) = terminal_response {
            run.response = Some(response);
            run.terminal = true;
        }

        Ok(event)
    }
}

impl Default for AgentEventBroker {
    fn default() -> Self {
        Self::new(BrokerOptions::default())
    }
}

#[derive(Debug)]
struct BrokerInner {
    options: BrokerOptions,
    state: Mutex<BrokerState>,
}

#[derive(Debug, Default)]
struct BrokerState {
    next_subscriber_id: u64,
    runs: BTreeMap<String, RunState>,
}

#[derive(Debug, Default)]
struct RunState {
    stream_id: String,
    next_sequence: u64,
    terminal: bool,
    response: Option<AgentResponseEnvelope>,
    events: VecDeque<AgentEventEnvelope>,
    subscribers: BTreeMap<u64, mpsc::Sender<AgentEventEnvelope>>,
}

fn format_timestamp() -> Result<String, AgentBrokerError> {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .map_err(|error| AgentBrokerError::Timestamp(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use elegy_contracts::{AgentMessage, AgentMessageRole, AgentRequestContext};

    fn request_fixture() -> AgentRequestEnvelope {
        AgentRequestEnvelope {
            request_id: "request-1".to_string(),
            messages: vec![AgentMessage {
                message_id: "message-1".to_string(),
                role: AgentMessageRole::User,
                content: "Summarize the API surface".to_string(),
                name: None,
            }],
            context: AgentRequestContext {
                session_id: Some("session-1".to_string()),
                ..AgentRequestContext::default()
            },
            streaming_requested: true,
        }
    }

    struct SuccessfulAdapter;

    #[async_trait]
    impl AgentRequestAdapter for SuccessfulAdapter {
        async fn execute(
            &self,
            request: AgentRequestEnvelope,
            run: AgentRunHandle,
        ) -> Result<AgentResponseEnvelope, AgentAdapterError> {
            run.publish(
                AgentEventType::MessageDelta,
                AgentEventSource::Model,
                AgentEventPayload {
                    message_id: Some("message-2".to_string()),
                    role: Some(AgentMessageRole::Assistant),
                    delta_content: Some("The API".to_string()),
                    ..AgentEventPayload::default()
                },
                true,
                None,
            )
            .map_err(|error| {
                AgentAdapterError::new("broker_publish_failed", error.to_string())
            })?;

            run.publish(
                AgentEventType::MessageCompleted,
                AgentEventSource::Model,
                AgentEventPayload {
                    message_id: Some("message-2".to_string()),
                    role: Some(AgentMessageRole::Assistant),
                    content: Some("The API surface has three operations.".to_string()),
                    ..AgentEventPayload::default()
                },
                false,
                None,
            )
            .map_err(|error| {
                AgentAdapterError::new("broker_publish_failed", error.to_string())
            })?;

            Ok(AgentResponseEnvelope {
                request_id: request.request_id,
                messages: vec![AgentMessage {
                    message_id: "message-2".to_string(),
                    role: AgentMessageRole::Assistant,
                    content: "The API surface has three operations.".to_string(),
                    name: Some(run.stream_id().to_string()),
                }],
                ..AgentResponseEnvelope::default()
            })
        }
    }

    struct FailingAdapter;

    #[async_trait]
    impl AgentRequestAdapter for FailingAdapter {
        async fn execute(
            &self,
            _request: AgentRequestEnvelope,
            _run: AgentRunHandle,
        ) -> Result<AgentResponseEnvelope, AgentAdapterError> {
            Err(AgentAdapterError::new(
                "adapter_failed",
                "upstream provider rejected the request",
            ))
        }
    }

    #[tokio::test]
    async fn replay_only_contains_persisted_events() {
        let broker = AgentEventBroker::default();
        let run = broker.start_request(request_fixture()).expect("start request");

        run.publish(
            AgentEventType::MessageDelta,
            AgentEventSource::Model,
            AgentEventPayload {
                delta_content: Some("The API".to_string()),
                ..AgentEventPayload::default()
            },
            true,
            None,
        )
        .expect("publish delta");

        run.publish(
            AgentEventType::MessageCompleted,
            AgentEventSource::Model,
            AgentEventPayload {
                content: Some("The API surface has three operations.".to_string()),
                ..AgentEventPayload::default()
            },
            false,
            None,
        )
        .expect("publish completion");

        let subscription = broker
            .subscribe("request-1", SubscriptionOptions::default())
            .expect("subscribe to existing run");
        let replay_types: Vec<AgentEventType> = subscription
            .replay
            .iter()
            .map(|event| event.event_type)
            .collect();

        assert_eq!(
            replay_types,
            vec![
                AgentEventType::RequestAccepted,
                AgentEventType::RunStarted,
                AgentEventType::MessageCompleted,
            ]
        );
    }

    #[tokio::test]
    async fn successful_adapter_processes_request_and_persists_response() {
        let broker = AgentEventBroker::default();
        let response = broker
            .process_request(request_fixture(), &SuccessfulAdapter)
            .await
            .expect("process request through adapter");

        assert_eq!(response.request_id, "request-1");
        assert_eq!(response.run_id, "request-1");

        let events = broker.events_for_run("request-1").expect("load events");
        let event_types: Vec<AgentEventType> =
            events.iter().map(|event| event.event_type).collect();
        assert_eq!(
            event_types,
            vec![
                AgentEventType::RequestAccepted,
                AgentEventType::RunStarted,
                AgentEventType::MessageCompleted,
                AgentEventType::RunCompleted,
            ]
        );

        let persisted = broker
            .response_for_run("request-1")
            .expect("load persisted response")
            .expect("response should be stored");
        assert_eq!(persisted, response);
    }

    #[tokio::test]
    async fn failing_adapter_marks_run_failed() {
        let broker = AgentEventBroker::default();
        let error = broker
            .process_request(request_fixture(), &FailingAdapter)
            .await
            .expect_err("adapter failure should surface");

        assert!(matches!(
            error,
            AgentBrokerError::AdapterFailed(_, _)
        ));

        let response = broker
            .response_for_run("request-1")
            .expect("load failed response")
            .expect("failed response should be stored");
        assert_eq!(response.status, AgentResponseStatus::Failed);
        assert_eq!(response.error_code.as_deref(), Some("adapter_failed"));

        let events = broker.events_for_run("request-1").expect("load events");
        assert_eq!(
            events.last().map(|event| event.event_type),
            Some(AgentEventType::RunFailed)
        );
    }
}