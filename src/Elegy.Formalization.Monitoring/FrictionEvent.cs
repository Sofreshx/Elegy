using Elegy.Formalization.Core.Agentic;

namespace Elegy.Formalization.Monitoring;

public static class FrictionEvent
{
    public static AgenticEvent FromFrictionEntry(
        string title,
        string reason,
        string importance,
        string context,
        string? clusterId = null)
    {
        return new AgenticEvent
        {
            EventId = $"friction-{Guid.NewGuid():N}",
            Timestamp = DateTimeOffset.UtcNow,
            EntityKind = AgenticEntityKind.Skill,
            EntityId = "implementation-friction",
            Category = EventCategory.Friction,
            Severity = MapSeverity(importance),
            Message = title,
            Metadata = BuildMetadata(reason, context, clusterId)
        };
    }

    private static MonitoringSeverity MapSeverity(string importance) => importance.ToLowerInvariant() switch
    {
        "low" => MonitoringSeverity.Info,
        "medium" => MonitoringSeverity.Warning,
        "high" => MonitoringSeverity.Error,
        "critical" => MonitoringSeverity.Critical,
        _ => MonitoringSeverity.Info
    };

    private static IReadOnlyDictionary<string, string> BuildMetadata(
        string reason,
        string context,
        string? clusterId)
    {
        var metadata = new Dictionary<string, string>
        {
            ["reason"] = reason,
            ["context"] = context
        };

        if (!string.IsNullOrWhiteSpace(clusterId))
        {
            metadata["clusterId"] = clusterId;
        }

        return metadata;
    }
}
