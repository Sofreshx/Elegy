using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.DynamicSkills.Tests;

public sealed class DynamicSkillEngineValidateTests
{
    private readonly DynamicSkillEngine _engine = new(new DynamicSkillEngineOptions { IsEnabled = true });

    [Fact]
    public void Validate_Returns_Valid_For_Good_Definition()
    {
        var result = _engine.Validate(new SkillDefinition { Id = "valid-skill", Name = "valid-skill" });
        Assert.True(result.IsValid);
        Assert.Empty(result.Errors);
    }

    [Fact]
    public void Validate_Returns_Invalid_For_Empty_Name()
    {
        var result = _engine.Validate(new SkillDefinition { Id = "valid-skill", Name = "" });
        Assert.False(result.IsValid);
        Assert.NotEmpty(result.Errors);
    }

    [Fact]
    public void Validate_Uses_Canonical_Skill_Validator()
    {
        var result = _engine.Validate(new SkillDefinition
        {
            Id = "valid-skill",
            Name = "valid-skill",
            Governance = new SkillGovernanceMetadata
            {
                ApprovalRequirement = SkillApprovalRequirement.Required,
            },
        });

        Assert.False(result.IsValid);
        Assert.Contains("Skills that require approval must declare at least one policy reference.", result.Errors);
    }
}
