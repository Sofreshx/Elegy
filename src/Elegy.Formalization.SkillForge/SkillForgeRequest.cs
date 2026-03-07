using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

public sealed record SkillForgeRequest
{
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<SkillTrigger> Triggers { get; init; } = [];
    public IReadOnlyList<SkillConstraint> Constraints { get; init; } = [];
    public IReadOnlyList<string> DiscoveryKeywords { get; init; } = [];
}
