using Elegy.Formalization.Agents;

namespace Elegy.Formalization.AgentFactory;

public sealed record AgentCreateResult
{
    public bool Success { get; init; }
    public AgentDefinition? CreatedAgent { get; init; }
    public IReadOnlyList<string> Findings { get; init; } = [];
    public string? ErrorMessage { get; init; }
}
