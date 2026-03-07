namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillValidationResult
{
    public bool IsValid { get; init; }
    public IReadOnlyList<string> Errors { get; init; } = [];
}
