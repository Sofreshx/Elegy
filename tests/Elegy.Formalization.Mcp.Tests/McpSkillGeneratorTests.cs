using System.Text.Json;
using Xunit;
using Elegy.Formalization.Mcp;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpSkillGeneratorTests
{
    private readonly McpSkillGenerator _sut = new();

    private static McpAnalysisResult CreateTestAnalysis() => new()
    {
        ServerName = "test-server",
        Analyses =
        [
            CreateAnalysis("list-files", "List all files", true),
            CreateAnalysis("read-content", "Read file contents", true),
            CreateAnalysis("no-schema-tool", "Tool without schema", false),
        ]
    };

    private static McpToolAnalysis CreateAnalysis(string name, string desc, bool validSchema)
    {
        var tool = new McpToolDefinition
        {
            Name = name,
            Description = desc,
            InputSchema = validSchema ? JsonDocument.Parse("{}").RootElement : null
        };

        return new McpToolAnalysis
        {
            Tool = tool,
            ExtractedTriggers = [new Skills.SkillTrigger { Pattern = name.Replace('-', ' ') }],
            HasValidSchema = validSchema,
        };
    }

    [Fact]
    public void Generate_ValidTools_CreatesSkillsWithMcpPrefix()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        Assert.Equal(2, result.GeneratedSkills.Count);
        Assert.All(result.GeneratedSkills, s => Assert.StartsWith("mcp-", s.Id));
    }

    [Fact]
    public void Generate_InvalidSchema_SkipsTools()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        Assert.Single(result.SkippedTools);
        Assert.Equal("no-schema-tool", result.SkippedTools[0].Name);
    }

    [Fact]
    public void Generate_OriginConstraint_Present()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        foreach (var skill in result.GeneratedSkills)
        {
            var origin = Assert.Single(skill.Constraints, c => c.ConstraintId == "origin");
            Assert.Equal("mcp-generated", origin.Description);
        }
    }

    [Fact]
    public void Generate_LifecycleState_IsDraft()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        Assert.All(result.GeneratedSkills, s =>
            Assert.Equal(Skills.SkillLifecycleState.Draft, s.LifecycleState));
    }
}
