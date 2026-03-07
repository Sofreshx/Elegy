using Elegy.Formalization.Agents;

namespace Elegy.Formalization.AgentFactory;

public sealed record AgentCreateRequest
{
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<AgentCapability> Capabilities { get; init; } = [];
    public IReadOnlyList<RoutingRule> RoutingRules { get; init; } = [];
    public AgentScope Scope { get; init; } = AgentScope.Session;
}
