using System.Text.Json;
using Xunit;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpToolAnalyzerTests
{
    private readonly McpToolAnalyzer _analyzer = new();

    [Fact]
    public void Analyze_tool_with_valid_schema_extracts_triggers_and_marks_valid()
    {
        var descriptor = new McpServerDescriptor
        {
            ServerName = "test-server",
            Tools =
            [
                new McpToolDefinition
                {
                    Name = "get-user",
                    Description = "Gets a user",
                    InputSchema = JsonDocument.Parse("""{ "type": "object" }""").RootElement
                }
            ]
        };

        var result = _analyzer.Analyze(descriptor);

        Assert.Equal("test-server", result.ServerName);
        Assert.Single(result.Analyses);

        var analysis = result.Analyses[0];
        Assert.True(analysis.HasValidSchema);
        Assert.Single(analysis.ExtractedTriggers);
        Assert.Equal("get user", analysis.ExtractedTriggers[0].Pattern);
        Assert.Equal("Extracted from MCP tool name", analysis.ExtractedTriggers[0].Description);
    }

    [Fact]
    public void Analyze_tool_without_schema_marks_invalid()
    {
        var descriptor = new McpServerDescriptor
        {
            ServerName = "no-schema-server",
            Tools =
            [
                new McpToolDefinition
                {
                    Name = "listItems",
                    Description = "Lists items"
                }
            ]
        };

        var result = _analyzer.Analyze(descriptor);

        var analysis = result.Analyses[0];
        Assert.False(analysis.HasValidSchema);
        Assert.Single(analysis.ExtractedTriggers);
        Assert.Equal("list items", analysis.ExtractedTriggers[0].Pattern);
    }

    [Fact]
    public void Analyze_mixed_tools_returns_correct_count_and_results()
    {
        var schema = JsonDocument.Parse("""{ "type": "object" }""").RootElement;

        var descriptor = new McpServerDescriptor
        {
            ServerName = "mixed-server",
            Tools =
            [
                new McpToolDefinition
                {
                    Name = "get-user",
                    InputSchema = schema
                },
                new McpToolDefinition
                {
                    Name = "create_item",
                    Description = "Creates an item"
                },
                new McpToolDefinition
                {
                    Name = "fetchOrderDetails",
                    InputSchema = schema
                }
            ]
        };

        var result = _analyzer.Analyze(descriptor);

        Assert.Equal("mixed-server", result.ServerName);
        Assert.Equal(3, result.Analyses.Count);

        // kebab-case with schema
        Assert.True(result.Analyses[0].HasValidSchema);
        Assert.Equal("get user", result.Analyses[0].ExtractedTriggers[0].Pattern);

        // snake_case without schema
        Assert.False(result.Analyses[1].HasValidSchema);
        Assert.Equal("create item", result.Analyses[1].ExtractedTriggers[0].Pattern);

        // camelCase with schema
        Assert.True(result.Analyses[2].HasValidSchema);
        Assert.Equal("fetch order details", result.Analyses[2].ExtractedTriggers[0].Pattern);
    }
}
