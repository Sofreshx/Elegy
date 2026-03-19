namespace Elegy.Formalization.Skills;

public sealed record SkillParameter
{
    public string Name { get; init; } = string.Empty;
    public string Type { get; init; } = "string";
    public string? Description { get; init; }
    public bool Required { get; init; } = true;
}