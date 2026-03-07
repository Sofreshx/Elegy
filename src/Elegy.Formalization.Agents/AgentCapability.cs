namespace Elegy.Formalization.Agents;

public sealed record AgentCapability
{
    public string CapabilityId { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
}
