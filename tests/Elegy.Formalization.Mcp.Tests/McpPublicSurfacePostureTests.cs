using System;
using Xunit;

namespace Elegy.Formalization.Mcp.Tests;

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
        Assert.True(typeof(IMcpFormalizationFacade).IsPublic);
        Assert.True(typeof(McpFormalizationFacade).IsPublic);
        Assert.True(typeof(McpSkillProjectionResult).IsPublic);
        Assert.True(typeof(McpToolSearchResult).IsPublic);
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
}
