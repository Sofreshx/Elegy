using Elegy.Formalization.AgentFactory;
using Elegy.Formalization.Agents;
using Xunit;

namespace Elegy.Formalization.AgentFactory.Tests;

public sealed class AgentFactoryServiceTests
{
    private static AgentFactoryService CreateService() =>
        new(new AgentFactoryOptions());

    private static AgentCreateRequest CreateValidRequest() =>
        new()
        {
            Name = "test-agent",
            Description = "Test",
            Capabilities = [new AgentCapability { CapabilityId = "cap-1", Name = "Cap One" }],
            RoutingRules = [],
            Scope = AgentScope.Session
        };

    [Fact]
    public void Create_ValidRequest_ReturnsSuccess()
    {
        var svc = CreateService();
        var result = svc.Create(CreateValidRequest());

        Assert.True(result.Success);
        Assert.NotNull(result.CreatedAgent);
        Assert.Empty(result.Findings);
        Assert.Equal("test-agent", result.CreatedAgent!.Name);
    }

    [Fact]
    public void Create_NonKebabName_ReturnsFindings()
    {
        var svc = CreateService();
        var request = CreateValidRequest() with { Name = "InvalidName" };

        var result = svc.Create(request);

        Assert.False(result.Success);
        Assert.Contains("Name must match kebab-case pattern.", result.Findings);
    }

    [Fact]
    public void Create_EmptyName_ReturnsFindings()
    {
        var svc = CreateService();
        var request = CreateValidRequest() with { Name = "" };

        var result = svc.Create(request);

        Assert.False(result.Success);
        Assert.Contains("Name is required.", result.Findings);
    }

    [Fact]
    public void Create_MissingDescription_ReturnsFindings()
    {
        var svc = CreateService();
        var request = CreateValidRequest() with { Description = null };

        var result = svc.Create(request);

        Assert.False(result.Success);
        Assert.Contains("Description is required.", result.Findings);
    }

    [Fact]
    public void Create_NoCapabilities_ReturnsFindings()
    {
        var svc = CreateService();
        var request = CreateValidRequest() with { Capabilities = [] };

        var result = svc.Create(request);

        Assert.False(result.Success);
        Assert.Contains("At least one capability is required.", result.Findings);
    }

    [Fact]
    public void Create_DuplicateCapabilityIds_ReturnsFindings()
    {
        var svc = CreateService();
        var request = CreateValidRequest() with
        {
            Capabilities =
            [
                new AgentCapability { CapabilityId = "dup", Name = "A" },
                new AgentCapability { CapabilityId = "dup", Name = "B" }
            ]
        };

        var result = svc.Create(request);

        Assert.False(result.Success);
        Assert.Contains("Duplicate capability ID: dup", result.Findings);
    }

    [Fact]
    public void Create_DuplicateRoutingRuleIds_ReturnsFindings()
    {
        var svc = CreateService();
        var request = CreateValidRequest() with
        {
            RoutingRules =
            [
                new RoutingRule { RuleId = "r1", Pattern = "a", Priority = 1, TargetCapabilityId = "cap-1" },
                new RoutingRule { RuleId = "r1", Pattern = "b", Priority = 2, TargetCapabilityId = "cap-1" }
            ]
        };

        var result = svc.Create(request);

        Assert.False(result.Success);
        Assert.Contains("Duplicate routing rule ID: r1", result.Findings);
    }

    [Fact]
    public void Validate_ValidDefinition_ReturnsValid()
    {
        var svc = CreateService();
        var definition = new AgentDefinition
        {
            Id = "id-1",
            Name = "my-agent",
            Description = "Valid",
            Capabilities = [new AgentCapability { CapabilityId = "c1", Name = "C" }],
            RoutingRules = []
        };

        var result = svc.Validate(definition);

        Assert.True(result.IsValid);
        Assert.Empty(result.Findings);
    }

    [Fact]
    public void Validate_InvalidDefinition_ReturnsFindings()
    {
        var svc = CreateService();
        var definition = new AgentDefinition
        {
            Id = "id-1",
            Name = "BAD NAME",
            Description = null,
            Capabilities = [],
            RoutingRules = []
        };

        var result = svc.Validate(definition);

        Assert.False(result.IsValid);
        Assert.Contains("Name must match kebab-case pattern.", result.Findings);
        Assert.Contains("Description is required.", result.Findings);
        Assert.Contains("At least one capability is required.", result.Findings);
    }
}
