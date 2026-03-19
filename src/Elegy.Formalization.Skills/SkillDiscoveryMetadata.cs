namespace Elegy.Formalization.Skills;

public sealed record SkillDiscoveryMetadata
{
    public IReadOnlyList<string> Keywords { get; init; } = [];
    public IReadOnlyList<string> CapabilityHints { get; init; } = [];
    public bool IsHidden { get; init; }
}