using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;
using Elegy.Formalization.SkillForge;
using Xunit;

namespace Elegy.Formalization.SkillForge.Tests;

public sealed class SkillForgeServiceTests
{
    private static SkillForgeRequest CreateValidRequest() => new()
    {
        Name = "my-skill",
        Description = "A test skill",
        Triggers = [new SkillTrigger { Pattern = "test trigger", Description = "fires on test" }],
        Constraints = [new SkillConstraint { ConstraintId = "c1", Description = "must be valid", Required = true }],
        DiscoveryKeywords = ["testing", "unit"]
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
        var request = CreateValidRequest() with { Name = badName };

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
        Assert.Equal(["testing", "unit"], result.RegistrationMetadata.DiscoveryKeywords);
    }
}
