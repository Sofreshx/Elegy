namespace Elegy.Formalization.Agents;

public sealed record RoutingRule
{
    public string RuleId { get; init; } = string.Empty;
    public string Pattern { get; init; } = string.Empty;
    public int Priority { get; init; }
    public string TargetCapabilityId { get; init; } = string.Empty;
}
