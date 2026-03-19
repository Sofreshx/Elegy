namespace Elegy.Formalization.Skills;

public sealed record SkillIdentity
{
    public string DefinitionId { get; init; } = string.Empty;
    public string DisplayName { get; init; } = string.Empty;
    public string? Namespace { get; init; }
    public string? Version { get; init; }
    public IReadOnlyList<string> Aliases { get; init; } = [];
}