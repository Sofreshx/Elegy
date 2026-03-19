namespace Elegy.Formalization.Skills;

public sealed record SkillOutputContract
{
    public string? ResultType { get; init; }
    public string? SchemaRef { get; init; }
    public bool ReturnsCollection { get; init; }
    public string? Description { get; init; }
}