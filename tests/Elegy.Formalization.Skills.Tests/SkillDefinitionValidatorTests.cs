using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.Skills.Tests;

public sealed class SkillDefinitionValidatorTests
{
    [Fact]
    public void Legacy_Skill_Shape_Is_Still_Valid()
    {
        var definition = new SkillDefinition
        {
            Id = "skill.example",
            Name = "Example skill",
        };

        var result = SkillDefinitionValidator.Validate(definition);

        Assert.True(result.IsValid);
        Assert.Empty(result.Errors);
    }

    [Fact]
    public void Rejects_Missing_Effective_Id_And_Name()
    {
        var definition = new SkillDefinition();

        var result = SkillDefinitionValidator.Validate(definition);

        Assert.False(result.IsValid);
        Assert.Contains("Skill definition ID is required.", result.Errors);
        Assert.Contains("Skill name is required.", result.Errors);
    }

    [Fact]
    public void Rejects_Duplicate_Input_Parameter_Names()
    {
        var definition = CreateValidDefinition() with
        {
            Input = new SkillInputContract
            {
                Parameters =
                [
                    new SkillParameter { Name = "query", Type = "string" },
                    new SkillParameter { Name = "Query", Type = "string" },
                ],
            },
        };

        var result = SkillDefinitionValidator.Validate(definition);

        Assert.False(result.IsValid);
        Assert.Contains("Skill input parameter names must be unique.", result.Errors);
    }

    [Fact]
    public void Requires_Policy_Refs_When_Approval_Is_Required()
    {
        var definition = CreateValidDefinition() with
        {
            Governance = new SkillGovernanceMetadata
            {
                ApprovalRequirement = SkillApprovalRequirement.Required,
            },
        };

        var result = SkillDefinitionValidator.Validate(definition);

        Assert.False(result.IsValid);
        Assert.Contains("Skills that require approval must declare at least one policy reference.", result.Errors);
    }

    [Fact]
    public void Requires_Dynamic_Skills_To_Record_Origin_Context()
    {
        var definition = CreateValidDefinition() with
        {
            Origin = new SkillOrigin
            {
                MaterializationKind = SkillMaterializationKind.Dynamic,
                SourceKind = SkillSourceKind.Manual,
            },
        };

        var result = SkillDefinitionValidator.Validate(definition);

        Assert.False(result.IsValid);
        Assert.Contains("Dynamic skills must declare either a source reference or a non-manual source kind.", result.Errors);
    }

    [Fact]
    public void Accepts_Dynamic_Skills_With_Generated_Origin()
    {
        var definition = CreateValidDefinition() with
        {
            Origin = new SkillOrigin
            {
                MaterializationKind = SkillMaterializationKind.Dynamic,
                SourceKind = SkillSourceKind.Generated,
            },
        };

        var result = SkillDefinitionValidator.Validate(definition);

        Assert.True(result.IsValid);
    }

    private static SkillDefinition CreateValidDefinition()
    {
        return new SkillDefinition
        {
            Id = "skill.example",
            Name = "Example skill",
            Input = new SkillInputContract
            {
                Parameters =
                [
                    new SkillParameter { Name = "query", Type = "string" },
                ],
            },
            Governance = new SkillGovernanceMetadata
            {
                ApprovalRequirement = SkillApprovalRequirement.None,
            },
            Origin = new SkillOrigin
            {
                MaterializationKind = SkillMaterializationKind.Declared,
                SourceKind = SkillSourceKind.Manual,
            },
        };
    }
}