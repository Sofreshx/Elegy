using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

public sealed record SkillForgeResult
{
    public bool Success { get; init; }
    public SkillDefinition? CreatedSkill { get; init; }
    public IReadOnlyList<string> GovernanceFindings { get; init; } = [];
    public RegistrationMetadata? RegistrationMetadata { get; init; }
    public string? ErrorMessage { get; init; }
}
