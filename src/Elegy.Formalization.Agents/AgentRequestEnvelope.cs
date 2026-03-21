namespace Elegy.Formalization.Agents;

public sealed record AgentRequestEnvelope
{
    public string RequestId { get; init; } = string.Empty;
    public IReadOnlyList<AgentMessage> Messages { get; init; } = [];
    public AgentRequestContext Context { get; init; } = new();
    public bool StreamingRequested { get; init; } = true;
}