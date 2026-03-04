namespace Elegy.Formalization.Governance;

public sealed record GovernancePolicy
{
    public string PolicyId { get; init; } = string.Empty;

    public GovernanceResolutionStrategy Strategy { get; init; } = GovernanceResolutionStrategy.Reconcile;

    public GovernanceEnforcementMode EnforcementMode { get; init; } = GovernanceEnforcementMode.Strict;

    public string? Description { get; init; }
}
