namespace Elegy.Formalization.Skills;

public sealed record SkillConstraint
{
    public string ConstraintId { get; init; } = string.Empty;
    public string? Description { get; init; }
    public bool Required { get; init; }
}
