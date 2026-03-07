namespace Elegy.Formalization.Skills.Discovery;

public sealed record SkillDiscoveryIndex
{
    public int SchemaVersion { get; init; } = 1;
    public IReadOnlyList<SkillIndexEntry> Entries { get; init; } = [];
    public DateTimeOffset BuiltAt { get; init; } = DateTimeOffset.MinValue;
}
