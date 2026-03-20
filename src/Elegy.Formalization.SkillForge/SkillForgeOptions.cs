namespace Elegy.Formalization.SkillForge;

[Obsolete("Compatibility surface only. Prefer governed artifacts or consumer-local configuration for durable forge options.")]
public sealed record SkillForgeOptions
{
    public string NamingPattern { get; init; } = @"^[a-z0-9]+(-[a-z0-9]+)*$";
    public bool RequireGovernanceBar { get; init; } = true;
}
