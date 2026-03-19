namespace Elegy.Formalization.Skills;

public sealed record SkillValidationResult
{
    public IReadOnlyList<string> Errors { get; init; } = [];

    public bool IsValid => Errors.Count == 0;
}