namespace Elegy.Formalization.Agents;

public sealed record AgentResponseEnvelope
{
    public string RequestId { get; init; } = string.Empty;
    public string RunId { get; init; } = string.Empty;
    public AgentResponseStatus Status { get; init; } = AgentResponseStatus.Completed;
    public IReadOnlyList<AgentMessage> Messages { get; init; } = [];
    public AgentUsage Usage { get; init; } = new();
    public string? ErrorCode { get; init; }
    public string? ErrorMessage { get; init; }
    public IReadOnlyDictionary<string, string> Metadata { get; init; } = new Dictionary<string, string>();
}