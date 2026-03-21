namespace Elegy.Formalization.Agents;

public enum AgentEventType
{
    RequestAccepted = 0,
    RunStarted = 1,
    MessageDelta = 2,
    MessageCompleted = 3,
    ReasoningDelta = 4,
    ReasoningCompleted = 5,
    ToolCallStarted = 6,
    ToolCallCompleted = 7,
    Warning = 8,
    Error = 9,
    RunCompleted = 10,
    RunFailed = 11,
    RunCancelled = 12,
}