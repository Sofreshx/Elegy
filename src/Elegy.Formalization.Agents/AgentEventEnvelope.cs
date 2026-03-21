namespace Elegy.Formalization.Agents;

public sealed record AgentEventEnvelope
{
    public string EventId { get; init; } = string.Empty;
    public string RunId { get; init; } = string.Empty;
    public string StreamId { get; init; } = string.Empty;
    public ulong Sequence { get; init; }
    public string? ParentEventId { get; init; }
    public DateTimeOffset Timestamp { get; init; }
    public bool Ephemeral { get; init; }
    public AgentEventType EventType { get; init; }
    public AgentEventSource Source { get; init; } = AgentEventSource.Broker;
    public AgentEventPayload Payload { get; init; } = new();
}