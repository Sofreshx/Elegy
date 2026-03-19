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
        Assert.All(result.GeneratedSkills, s => Assert.StartsWith("mcp-", s.EffectiveId));
    }

    [Fact]
    public void Generate_InvalidSchema_SkipsTools()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        Assert.Single(result.SkippedTools);
        Assert.Equal("no-schema-tool", result.SkippedTools[0].Name);
    }

    [Fact]
    public void Generate_Uses_Canonical_Origin_And_Discovery_Metadata()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        foreach (var skill in result.GeneratedSkills)
        {
            Assert.Equal(Skills.SkillSourceKind.Generated, skill.Origin.SourceKind);
            Assert.Equal(Skills.SkillMaterializationKind.Declared, skill.Origin.MaterializationKind);
            Assert.StartsWith("mcp://test-server/tools/", skill.Origin.SourceRef);
            Assert.Contains("test-server", skill.Discovery.Keywords);
        }
    }

    [Fact]
    public void Generate_LifecycleState_IsDraft()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        Assert.All(result.GeneratedSkills, s =>
            Assert.Equal(Skills.SkillLifecycleState.Draft, s.LifecycleState));
    }

    [Fact]
    public void Generate_InputSchemaRef_Present_When_Tool_Has_Schema()
    {
        var result = _sut.Generate(CreateTestAnalysis());

        Assert.All(result.GeneratedSkills, skill => Assert.Contains("/input-schema", skill.Input.SchemaRef));
    }
}
