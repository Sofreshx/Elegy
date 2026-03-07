using Elegy.Formalization.Skills;

namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillCreateResult
{
    public bool Success { get; init; }
    public SkillDefinition? CreatedSkill { get; init; }
    public string? ErrorMessage { get; init; }
}
