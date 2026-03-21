namespace Elegy.Formalization.Agents;

public sealed record AgentRequestContext
{
    public string CorrelationId { get; init; } = string.Empty;
    public string? SessionId { get; init; }
    public string? ConversationId { get; init; }
    public string? RequestedSkillId { get; init; }
    public IReadOnlyList<string> CapabilityHints { get; init; } = [];
    public IReadOnlyDictionary<string, string> Metadata { get; init; } = new Dictionary<string, string>();
}