using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

[Obsolete("Compatibility surface only. Prefer governed artifacts for durable registration metadata contracts.")]
public sealed record RegistrationMetadata
{
    public string ManifestEntry { get; init; } = string.Empty;
    public string SkillId { get; init; } = string.Empty;
    public IReadOnlyList<string> DiscoveryKeywords { get; init; } = [];
    public SkillSourceKind SourceKind { get; init; } = SkillSourceKind.Manual;
    public SkillMaterializationKind MaterializationKind { get; init; } = SkillMaterializationKind.Declared;
}
