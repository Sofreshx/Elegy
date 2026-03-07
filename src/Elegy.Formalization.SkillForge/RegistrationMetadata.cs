namespace Elegy.Formalization.SkillForge;

public sealed record RegistrationMetadata
{
    public string ManifestEntry { get; init; } = string.Empty;
    public IReadOnlyList<string> DiscoveryKeywords { get; init; } = [];
}
