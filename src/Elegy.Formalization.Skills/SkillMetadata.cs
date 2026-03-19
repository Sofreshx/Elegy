namespace Elegy.Formalization.Skills;

public sealed record SkillMetadata
{
    public string? Summary { get; init; }
    public string? Category { get; init; }
    public IReadOnlyList<string> Tags { get; init; } = [];
    public IReadOnlyList<string> Owners { get; init; } = [];
    public string? DocumentationUri { get; init; }
}