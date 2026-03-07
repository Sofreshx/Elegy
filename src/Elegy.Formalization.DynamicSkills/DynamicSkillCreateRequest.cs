using Elegy.Formalization.Skills;

namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillCreateRequest
{
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<SkillTrigger> Triggers { get; init; } = [];
    public IReadOnlyList<SkillConstraint> Constraints { get; init; } = [];
}
