using System.Text.Json;
using Xunit;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpFormalizationFacadeTests
{
    private readonly IMcpFormalizationFacade _sut = new McpFormalizationFacade();

    [Fact]
    public void Analyze_And_ProjectSkills_Wrap_Internal_Helper_Pipeline()
    {
        var descriptor = CreateDescriptor();

        var analysis = _sut.Analyze(descriptor);
        var projection = _sut.ProjectSkills(analysis);

        Assert.Equal("demo-server", analysis.ServerName);
        Assert.Equal(3, analysis.Analyses.Count);
        Assert.Equal(2, projection.GeneratedSkills.Count);
        Assert.Single(projection.SkippedTools);
        Assert.Equal("no-schema-tool", projection.SkippedTools[0].Name);
    }

    [Fact]
    public void ProjectSkills_Descriptor_Overload_Performs_Analysis_First()
    {
        var projection = _sut.ProjectSkills(CreateDescriptor());

        Assert.Equal(2, projection.GeneratedSkills.Count);
        Assert.All(projection.GeneratedSkills, static skill => Assert.StartsWith("mcp-", skill.EffectiveId));
    }

    [Fact]
    public void SearchTools_And_ResolveTool_Return_Public_Consumer_Shapes()
    {
        var descriptor = CreateDescriptor();

        var searchResults = _sut.SearchTools(descriptor, "file");
        var resolved = _sut.ResolveTool(descriptor, "read-file");

        Assert.Equal(2, searchResults.Count);
        Assert.Contains(searchResults, static result => result.Name == "list-files");
        Assert.Contains(searchResults, static result => result.Name == "read-file");
        Assert.NotNull(resolved);
        Assert.Equal("read-file", resolved!.Name);
        Assert.NotNull(resolved.InputSchema);
    }

    private static McpServerDescriptor CreateDescriptor()
    {
        return new McpServerDescriptor
        {
            ServerName = "demo-server",
            Transport = McpTransportKind.Stdio,
            Tools =
            [
                new McpToolDefinition
                {
                    Name = "list-files",
                    Description = "List files in a directory",
                    InputSchema = JsonDocument.Parse("""{ "type": "object" }""").RootElement
                },
                new McpToolDefinition
                {
                    Name = "read-file",
                    Description = "Read a file by path",
                    InputSchema = JsonDocument.Parse("""{ "type": "object" }""").RootElement
                },
                new McpToolDefinition
                {
                    Name = "no-schema-tool",
                    Description = "Skipped because schema is missing"
                }
            ]
        };
    }
}
