using System;
using Xunit;

namespace Elegy.Formalization.Mcp.Tests;

#pragma warning disable CS0618

public sealed class McpPublicSurfacePostureTests
{
    [Fact]
    public void Authoritative_contract_types_remain_public()
    {
        Assert.True(typeof(McpServerDescriptor).IsPublic);
        Assert.True(typeof(McpToolDefinition).IsPublic);
        Assert.True(typeof(McpAnalysisResult).IsPublic);
        Assert.True(typeof(McpToolAnalysis).IsPublic);
        Assert.True(typeof(McpTransportKind).IsPublic);
    }

    [Fact]
    public void Compatibility_shell_types_remain_public_for_transition()
    {
        Assert.True(typeof(IMcpFormalizationFacade).IsPublic);
        Assert.True(typeof(McpFormalizationFacade).IsPublic);
        Assert.True(typeof(McpSkillProjectionResult).IsPublic);
        Assert.True(typeof(McpToolSearchResult).IsPublic);
    }

    [Fact]
    public void Compatibility_shell_types_are_marked_obsolete()
    {
        AssertCompatibilitySurface(typeof(IMcpFormalizationFacade));
        AssertCompatibilitySurface(typeof(McpFormalizationFacade));
        AssertCompatibilitySurface(typeof(McpSkillProjectionResult));
        AssertCompatibilitySurface(typeof(McpToolSearchResult));
    }

    [Fact]
    public void Duplicated_behavior_types_are_internal_only()
    {
        Assert.False(typeof(McpToolAnalyzer).IsPublic);
        Assert.False(typeof(McpSkillGenerator).IsPublic);
        Assert.False(typeof(McpToolSearchService).IsPublic);
        Assert.False(typeof(McpToolResolveService).IsPublic);
        Assert.False(typeof(McpSkillGenerationResult).IsPublic);
        Assert.False(typeof(McpToolSummary).IsPublic);
    }

    private static void AssertCompatibilitySurface(Type type)
    {
        var attribute = (ObsoleteAttribute?)Attribute.GetCustomAttribute(type, typeof(ObsoleteAttribute));
        Assert.NotNull(attribute);
        Assert.Contains("Compatibility surface only", attribute!.Message, StringComparison.Ordinal);
    }
}

#pragma warning restore CS0618
