namespace Elegy.Formalization.Skills;

public sealed record SkillOrigin
{
    public SkillMaterializationKind MaterializationKind { get; init; } = SkillMaterializationKind.Declared;
    public SkillSourceKind SourceKind { get; init; } = SkillSourceKind.Manual;
    public string? SourceRef { get; init; }
    public string? SourceVersion { get; init; }
}