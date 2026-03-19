using Elegy.Formalization.Skills;

namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillCreateRequest
{
    public string SkillId { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<SkillTrigger> Triggers { get; init; } = [];
    public IReadOnlyList<SkillConstraint> Constraints { get; init; } = [];
    public SkillIdentity Identity { get; init; } = new();
    public SkillMetadata Metadata { get; init; } = new();
    public SkillInputContract Input { get; init; } = new();
    public SkillOutputContract Output { get; init; } = new();
    public SkillExecutionContract Execution { get; init; } = new();
    public SkillGovernanceMetadata Governance { get; init; } = new();
    public SkillDiscoveryMetadata Discovery { get; init; } = new();
    public SkillOrigin Origin { get; init; } = new();
    public SkillLifecycleState LifecycleState { get; init; } = SkillLifecycleState.Draft;
}
