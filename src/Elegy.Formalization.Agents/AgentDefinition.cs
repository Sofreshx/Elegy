namespace Elegy.Formalization.Agents;

public sealed record AgentDefinition
{
    public string Id { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<AgentCapability> Capabilities { get; init; } = [];
    public IReadOnlyList<RoutingRule> RoutingRules { get; init; } = [];
    public AgentScope Scope { get; init; } = AgentScope.Session;
}
