using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.DynamicSkills.Tests;

#pragma warning disable CS0618

public sealed class DynamicSkillEngineCreateTests
{
    private readonly DynamicSkillEngine _engine = new(new DynamicSkillEngineOptions { IsEnabled = true });

    [Fact]
    public void Create_Returns_Success_With_Valid_Request()
    {
        var result = _engine.Create(new DynamicSkillCreateRequest { SkillId = "my-skill", Name = "my-skill" });
        Assert.True(result.Success);
        Assert.NotNull(result.CreatedSkill);
        Assert.Equal("my-skill", result.CreatedSkill!.Name);
        Assert.Equal("my-skill", result.CreatedSkill.EffectiveId);
        Assert.Equal(SkillMaterializationKind.Dynamic, result.CreatedSkill.Origin.MaterializationKind);
    }

    [Fact]
    public void Create_Returns_Failure_With_Empty_Name()
    {
        var result = _engine.Create(new DynamicSkillCreateRequest { Name = "" });
        Assert.False(result.Success);
        Assert.NotNull(result.ErrorMessage);
        Assert.False(result.Validation.IsValid);
    }

    [Fact]
    public void Create_Preserves_Canonical_Blocks_And_Normalizes_Dynamic_Origin()
    {
        var result = _engine.Create(new DynamicSkillCreateRequest
        {
            SkillId = "canonical-skill",
            Name = "Canonical Skill",
            Description = "A canonical dynamic skill",
            Metadata = new SkillMetadata { Category = "testing" },
            Input = new SkillInputContract { Parameters = [new SkillParameter { Name = "query", Type = "string" }] },
            Governance = new SkillGovernanceMetadata { AllowedContexts = ["workspace"] },
            Origin = new SkillOrigin { SourceKind = SkillSourceKind.Imported, SourceRef = "import://catalog/canonical-skill" },
        });

        Assert.True(result.Success);
        Assert.NotNull(result.CreatedSkill);
        Assert.Equal("canonical-skill", result.CreatedSkill!.EffectiveId);
        Assert.Equal("Canonical Skill", result.CreatedSkill.EffectiveName);
        Assert.Equal("testing", result.CreatedSkill.Metadata.Category);
        Assert.Equal("query", Assert.Single(result.CreatedSkill.Input.Parameters).Name);
        Assert.Equal(SkillMaterializationKind.Dynamic, result.CreatedSkill.Origin.MaterializationKind);
        Assert.Equal(SkillSourceKind.Imported, result.CreatedSkill.Origin.SourceKind);
    }
}

#pragma warning restore CS0618
