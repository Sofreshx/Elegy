using Elegy.Formalization.Skills;

namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillValidationResult
{
    public SkillValidationResult Validation { get; init; } = new();
    public bool IsValid => Validation.IsValid;
    public IReadOnlyList<string> Errors => Validation.Errors;
}
