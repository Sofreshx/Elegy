using System.Text.Json;
using Xunit;
using Elegy.Formalization.Core.Agentic;
using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Mcp;
using Elegy.Formalization.Monitoring;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpDynamicSkillBridgeTests
{
    private static DynamicSkillEngine CreateEngine(bool enabled = true)
    {
        return new DynamicSkillEngine(new DynamicSkillEngineOptions { IsEnabled = enabled });
    }

    private static McpSkillGenerationResult CreateGenerationResult(int skillCount)
    {
        var skills = new List<SkillDefinition>();
        for (int i = 0; i < skillCount; i++)
        {
            skills.Add(new SkillDefinition
            {
                Id = $"mcp-server-tool{i}",
                Name = $"tool{i}",
                Description = $"Test tool {i}",
                Triggers = [new SkillTrigger { Pattern = $"tool{i}" }],
                Constraints = [new SkillConstraint { ConstraintId = "origin", Description = "mcp-generated" }],
                LifecycleState = SkillLifecycleState.Draft
            });
        }

        return new McpSkillGenerationResult
        {
            GeneratedSkills = skills,
            SkippedTools = []
        };
    }

    [Fact]
    public void RegisterMcpSkills_TwoSkills_ProducesTwoCreatesPlusTwoEvents()
    {
        var engine = CreateEngine();
        var bridge = new McpDynamicSkillBridge();
        var generationResult = CreateGenerationResult(2);

        var result = bridge.RegisterMcpSkills(engine, generationResult);

        Assert.Equal(2, result.Results.Count);
        Assert.Equal(2, result.Events.Count);

        Assert.All(result.Results, r => Assert.True(r.Success));
        Assert.All(result.Events, e =>
        {
            Assert.Equal(AgenticEntityKind.DynamicSkill, e.EntityKind);
            Assert.Equal(EventCategory.Lifecycle, e.Category);
            Assert.Equal(MonitoringSeverity.Info, e.Severity);
            Assert.NotNull(e.Metadata);
            Assert.Equal("mcp-generated", e.Metadata!["origin"]);
            Assert.NotEmpty(e.Metadata["mcpToolName"]);
        });
    }

    [Fact]
    public void RegisterMcpSkills_DisabledEngine_CapturesFailureWithoutThrowing()
    {
        var engine = CreateEngine(enabled: false);
        var bridge = new McpDynamicSkillBridge();
        var generationResult = CreateGenerationResult(1);

        var result = bridge.RegisterMcpSkills(engine, generationResult);

        Assert.Single(result.Results);
        Assert.False(result.Results[0].Success);
        Assert.NotNull(result.Results[0].ErrorMessage);

        Assert.Single(result.Events);
        Assert.Equal(MonitoringSeverity.Warning, result.Events[0].Severity);
    }

    [Fact]
    public void RegisterMcpSkills_EmptyGenerationResult_ReturnsEmptyBridgeResult()
    {
        var engine = CreateEngine();
        var bridge = new McpDynamicSkillBridge();
        var generationResult = new McpSkillGenerationResult
        {
            GeneratedSkills = [],
            SkippedTools = []
        };

        var result = bridge.RegisterMcpSkills(engine, generationResult);

        Assert.Empty(result.Results);
        Assert.Empty(result.Events);
    }

    [Fact]
    public void RegisterMcpSkills_EventMetadata_ContainsMcpToolName()
    {
        var engine = CreateEngine();
        var bridge = new McpDynamicSkillBridge();
        var generationResult = CreateGenerationResult(1);

        var result = bridge.RegisterMcpSkills(engine, generationResult);

        Assert.Single(result.Events);
        Assert.Equal("server-tool0", result.Events[0].Metadata!["mcpToolName"]);
    }
}
