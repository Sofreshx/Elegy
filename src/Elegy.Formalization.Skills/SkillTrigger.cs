namespace Elegy.Formalization.Skills;

public sealed record SkillTrigger
{
    public string Pattern { get; init; } = string.Empty;
    public string? Description { get; init; }
}
