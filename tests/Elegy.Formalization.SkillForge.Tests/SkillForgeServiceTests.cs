using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;
using Elegy.Formalization.SkillForge;
using Xunit;

namespace Elegy.Formalization.SkillForge.Tests;

#pragma warning disable CS0618

public sealed class SkillForgeServiceTests
{
    private static SkillForgeRequest CreateValidRequest() => new()
    {
        SkillId = "my-skill",
        Name = "my-skill",
        Description = "A test skill",
        Triggers = [new SkillTrigger { Pattern = "test trigger", Description = "fires on test" }],
        Constraints = [new SkillConstraint { ConstraintId = "c1", Description = "must be valid", Required = true }],
        DiscoveryKeywords = ["testing", "unit"],
        Input = new SkillInputContract
        {
            Parameters = [new SkillParameter { Name = "query", Type = "string", Description = "Search query" }]
        },
        Governance = new SkillGovernanceMetadata
        {
            AllowedContexts = ["workspace"]
        },
    };

    private static SkillForgeService CreateService(bool engineEnabled = true) =>
        new(
            new DynamicSkillEngine(new DynamicSkillEngineOptions { IsEnabled = engineEnabled }),
            new SkillForgeOptions()
        );

    [Fact]
    public void Forge_ValidRequest_ReturnsSuccess()
    {
        var service = CreateService();
        var result = service.Forge(CreateValidRequest());

        Assert.True(result.Success);
        Assert.NotNull(result.CreatedSkill);
        Assert.Equal("my-skill", result.CreatedSkill!.Name);
        Assert.Equal("my-skill", result.CreatedSkill.EffectiveId);
        Assert.Empty(result.GovernanceFindings);
        Assert.Null(result.ErrorMessage);
    }

    [Theory]
    [InlineData("My-Skill")]
    [InlineData("my skill")]
    [InlineData("MY_SKILL")]
    [InlineData("")]
    public void Forge_InvalidName_ReturnsFalseWithPattern(string badName)
    {
        var service = CreateService();
        var request = CreateValidRequest() with { SkillId = badName, Name = badName };

        var result = service.Forge(request);

        Assert.False(result.Success);
        Assert.Contains("does not match required pattern", result.ErrorMessage);
    }

    [Fact]
    public void Forge_MissingTriggers_ReturnsGovernanceFinding()
    {
        var service = CreateService();
        var request = CreateValidRequest() with { Triggers = [] };

        var result = service.Forge(request);

        Assert.False(result.Success);
        Assert.Contains("At least one trigger is required.", result.GovernanceFindings);
        Assert.Equal("Request does not meet the governance bar.", result.ErrorMessage);
    }

    [Fact]
    public void Forge_MissingConstraints_ReturnsGovernanceFinding()
    {
        var service = CreateService();
        var request = CreateValidRequest() with { Constraints = [] };

        var result = service.Forge(request);

        Assert.False(result.Success);
        Assert.Contains("At least one constraint is required.", result.GovernanceFindings);
        Assert.Equal("Request does not meet the governance bar.", result.ErrorMessage);
    }

    [Fact]
    public void Forge_MissingDescription_ReturnsGovernanceFinding()
    {
        var service = CreateService();
        var request = CreateValidRequest() with { Description = null };

        var result = service.Forge(request);

        Assert.False(result.Success);
        Assert.Contains("Description must not be empty.", result.GovernanceFindings);
        Assert.Equal("Request does not meet the governance bar.", result.ErrorMessage);
    }

    [Fact]
    public void Forge_DisabledEngine_ThrowsInvalidOperationException()
    {
        var service = CreateService(engineEnabled: false);
        var request = CreateValidRequest();

        Assert.Throws<InvalidOperationException>(() => service.Forge(request));
    }

    [Fact]
    public void Forge_Success_PopulatesRegistrationMetadata()
    {
        var service = CreateService();
        var request = CreateValidRequest();

        var result = service.Forge(request);

        Assert.True(result.Success);
        Assert.NotNull(result.RegistrationMetadata);
        Assert.Equal("my-skill", result.RegistrationMetadata!.ManifestEntry);
        Assert.Equal("my-skill", result.RegistrationMetadata.SkillId);
        Assert.Equal(["testing", "unit"], result.RegistrationMetadata.DiscoveryKeywords);
    }

    [Fact]
    public void Forge_Preserves_Canonical_Metadata_For_Downstream_Use()
    {
        var service = CreateService();
        var request = CreateValidRequest() with
        {
            Name = "My Skill",
            Identity = new SkillIdentity { DefinitionId = "my-skill", DisplayName = "My Skill" },
            Discovery = new SkillDiscoveryMetadata { CapabilityHints = ["lookup"] },
            Origin = new SkillOrigin { SourceKind = SkillSourceKind.Imported, SourceRef = "import://skills/my-skill" },
            Governance = new SkillGovernanceMetadata
            {
                RiskLevel = SkillRiskLevel.Medium,
                AllowedContexts = ["workspace"],
            },
        };

        var result = service.Forge(request);

        Assert.True(result.Success);
        Assert.NotNull(result.CreatedSkill);
        Assert.Equal("my-skill", result.CreatedSkill!.EffectiveId);
        Assert.Equal("My Skill", result.CreatedSkill.EffectiveName);
        Assert.Equal(SkillMaterializationKind.Dynamic, result.CreatedSkill.Origin.MaterializationKind);
        Assert.Equal(SkillSourceKind.Imported, result.CreatedSkill.Origin.SourceKind);
        Assert.Contains("lookup", result.CreatedSkill.Discovery.CapabilityHints);
        Assert.Equal(SkillRiskLevel.Medium, result.CreatedSkill.Governance.RiskLevel);
        Assert.True(result.Validation.IsValid);
    }
}

#pragma warning restore CS0618
