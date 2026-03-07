namespace Elegy.Formalization.SkillForge;

public sealed record SkillMaterializeResult
{
    public bool Success { get; init; }
    public string? WrittenPath { get; init; }
    public string? ErrorMessage { get; init; }
}
