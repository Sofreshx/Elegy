namespace Elegy.Formalization.Skills;

public sealed record SkillInputContract
{
    public IReadOnlyList<SkillParameter> Parameters { get; init; } = [];
    public string? SchemaRef { get; init; }
}