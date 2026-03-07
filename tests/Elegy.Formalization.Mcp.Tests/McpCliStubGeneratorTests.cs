using System.Text.Json;
using Xunit;
using Elegy.Formalization.Mcp;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpCliStubGeneratorTests
{
    private readonly McpCliStubGenerator _sut = new();

    [Fact]
    public void Generate_ObjectSchema_PopulatesArguments()
    {
        var schema = JsonDocument.Parse("""
        {
            "type": "object",
            "properties": {
                "path": { "type": "string", "description": "File path" },
                "recursive": { "type": "boolean" }
            },
            "required": ["path"]
        }
        """).RootElement;

        var analysis = new McpAnalysisResult
        {
            ServerName = "fs-server",
            Analyses =
            [
                new McpToolAnalysis
                {
                    Tool = new McpToolDefinition { Name = "list_files", InputSchema = schema },
                    ExtractedTriggers = [],
                    HasValidSchema = true
                }
            ]
        };

        var stubs = _sut.Generate(analysis);

        Assert.Single(stubs);
        Assert.Equal("list-files", stubs[0].CommandName);
        Assert.Equal(2, stubs[0].Arguments.Count);

        var pathArg = Assert.Single(stubs[0].Arguments, a => a.Name == "path");
        Assert.True(pathArg.Required);
        Assert.Equal("string", pathArg.Type);
        Assert.Equal("File path", pathArg.Description);

        var recursiveArg = Assert.Single(stubs[0].Arguments, a => a.Name == "recursive");
        Assert.False(recursiveArg.Required);
        Assert.Equal("boolean", recursiveArg.Type);
    }

    [Fact]
    public void Generate_NoSchema_YieldsEmptyArguments()
    {
        var analysis = new McpAnalysisResult
        {
            ServerName = "server",
            Analyses =
            [
                new McpToolAnalysis
                {
                    Tool = new McpToolDefinition { Name = "simple-tool" },
                    ExtractedTriggers = [],
                    HasValidSchema = false
                }
            ]
        };

        var stubs = _sut.Generate(analysis);

        Assert.Single(stubs);
        Assert.Empty(stubs[0].Arguments);
    }

    [Fact]
    public void Generate_CommandName_DerivedFromToolName()
    {
        var analysis = new McpAnalysisResult
        {
            ServerName = "server",
            Analyses =
            [
                new McpToolAnalysis
                {
                    Tool = new McpToolDefinition { Name = "Read_File_Content" },
                    ExtractedTriggers = [],
                    HasValidSchema = true
                }
            ]
        };

        var stubs = _sut.Generate(analysis);

        Assert.Single(stubs);
        Assert.Equal("read-file-content", stubs[0].CommandName);
    }
}
