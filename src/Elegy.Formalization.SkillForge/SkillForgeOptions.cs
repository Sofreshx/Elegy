namespace Elegy.Formalization.SkillForge;

public sealed record SkillForgeOptions
{
    public string NamingPattern { get; init; } = @"^[a-z0-9]+(-[a-z0-9]+)*$";
    public bool RequireGovernanceBar { get; init; } = true;
}
