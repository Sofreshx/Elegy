using Elegy.Formalization.Skills;

namespace Elegy.Formalization.DynamicSkills;

[Obsolete("Compatibility surface only. Prefer canonical skill authority or governed artifacts for durable contracts.")]
public sealed record DynamicSkillCreateResult
{
    public bool Success { get; init; }
    public SkillDefinition? CreatedSkill { get; init; }
    public SkillValidationResult Validation { get; init; } = new();
    public string? ErrorMessage { get; init; }
}
