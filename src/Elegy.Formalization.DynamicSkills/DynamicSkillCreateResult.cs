using Elegy.Formalization.Skills;

namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillCreateResult
{
    public bool Success { get; init; }
    public SkillDefinition? CreatedSkill { get; init; }
    public SkillValidationResult Validation { get; init; } = new();
    public string? ErrorMessage { get; init; }
}
