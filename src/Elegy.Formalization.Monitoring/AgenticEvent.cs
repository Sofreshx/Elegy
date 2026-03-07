using Elegy.Formalization.Core.Agentic;

namespace Elegy.Formalization.Monitoring;

public sealed record AgenticEvent
{
    public string EventId { get; init; } = string.Empty;
    public DateTimeOffset Timestamp { get; init; }
    public AgenticEntityKind EntityKind { get; init; }
    public string EntityId { get; init; } = string.Empty;
    public EventCategory Category { get; init; }
    public MonitoringSeverity Severity { get; init; }
    public string Message { get; init; } = string.Empty;
    public IReadOnlyDictionary<string, string>? Metadata { get; init; }
}
