namespace Elegy.Formalization.Skills.Discovery;

public sealed record SkillIndexEntry
{
    public string Id { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<string> Triggers { get; init; } = [];
    public SkillLoadMode LoadMode { get; init; } = SkillLoadMode.Always;
    public string? VaultRef { get; init; }
}
