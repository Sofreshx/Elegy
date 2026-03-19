using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Skills.Discovery;

public sealed record SkillIndexManifest
{
    public string Id { get; init; } = string.Empty;
    public SkillLoadMode LoadMode { get; init; } = SkillLoadMode.Always;
    public string? VaultRef { get; init; }
    public SkillSourceKind SourceKind { get; init; } = SkillSourceKind.Manual;
    public SkillMaterializationKind MaterializationKind { get; init; } = SkillMaterializationKind.Declared;
}