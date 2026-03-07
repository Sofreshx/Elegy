namespace Elegy.Formalization.Skills;

public sealed record SkillDefinition
{
    public string Id { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<SkillTrigger> Triggers { get; init; } = [];
    public IReadOnlyList<SkillConstraint> Constraints { get; init; } = [];
    public SkillLifecycleState LifecycleState { get; init; } = SkillLifecycleState.Draft;
}
