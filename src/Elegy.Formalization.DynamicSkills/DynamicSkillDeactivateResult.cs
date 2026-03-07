namespace Elegy.Formalization.DynamicSkills;

public sealed record DynamicSkillDeactivateResult
{
    public bool Success { get; init; }
    public string? ErrorMessage { get; init; }
}
