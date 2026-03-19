namespace Elegy.Formalization.Skills;

public sealed record SkillGovernanceMetadata
{
    public SkillRiskLevel RiskLevel { get; init; } = SkillRiskLevel.Low;
    public SkillApprovalRequirement ApprovalRequirement { get; init; } = SkillApprovalRequirement.None;
    public IReadOnlyList<string> PolicyRefs { get; init; } = [];
    public IReadOnlyList<string> AllowedContexts { get; init; } = [];
}