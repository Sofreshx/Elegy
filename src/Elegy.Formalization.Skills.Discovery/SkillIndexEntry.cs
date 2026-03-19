using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Skills.Discovery;

public sealed record SkillIndexEntry
{
    public string SkillId { get; init; } = string.Empty;
    public string Name { get; init; } = string.Empty;
    public string Description { get; init; } = string.Empty;
    public SkillLifecycleState LifecycleState { get; init; } = SkillLifecycleState.Draft;
    public IReadOnlyList<string> TriggersOn { get; init; } = [];
    public IReadOnlyList<string> Keywords { get; init; } = [];
    public IReadOnlyList<string> CapabilityHints { get; init; } = [];
    public SkillIndexManifest Manifest { get; init; } = new();

    public string Id => SkillId;
    public IReadOnlyList<string> Triggers => TriggersOn;
    public SkillLoadMode LoadMode => Manifest.LoadMode;
    public string? VaultRef => Manifest.VaultRef;

    public static SkillIndexEntry FromSkillDefinition(SkillDefinition definition, SkillIndexManifest manifest)
    {
        ArgumentNullException.ThrowIfNull(definition);
        ArgumentNullException.ThrowIfNull(manifest);

        return new SkillIndexEntry
        {
            SkillId = definition.EffectiveId,
            Name = definition.EffectiveName,
            Description = definition.Metadata.Summary ?? definition.Description ?? string.Empty,
            LifecycleState = definition.LifecycleState,
            TriggersOn = definition.Triggers
                .Select(static trigger => trigger.Pattern)
                .Where(static pattern => !string.IsNullOrWhiteSpace(pattern))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToArray(),
            Keywords = definition.Discovery.Keywords
                .Where(static keyword => !string.IsNullOrWhiteSpace(keyword))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToArray(),
            CapabilityHints = definition.Discovery.CapabilityHints
                .Where(static hint => !string.IsNullOrWhiteSpace(hint))
                .Distinct(StringComparer.OrdinalIgnoreCase)
                .ToArray(),
            Manifest = string.IsNullOrWhiteSpace(manifest.Id)
                ? manifest with { Id = definition.EffectiveId }
                : manifest,
        };
    }
}
