using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

[Obsolete("Compatibility surface only. Prefer governed artifacts for durable forge result contracts.")]
public sealed record SkillForgeResult
{
    public bool Success { get; init; }
    public SkillDefinition? CreatedSkill { get; init; }
    public SkillValidationResult Validation { get; init; } = new();
    public IReadOnlyList<string> GovernanceFindings { get; init; } = [];
    public RegistrationMetadata? RegistrationMetadata { get; init; }
    public string? ErrorMessage { get; init; }
}
