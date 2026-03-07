using System.Text.Json;
using Xunit;
using Elegy.Formalization.Mcp;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpToolDiscoveryTests
{
    private static McpServerDescriptor CreateTestDescriptor() => new()
    {
        ServerName = "test-server",
        Transport = McpTransportKind.Stdio,
        Tools =
        [
            new McpToolDefinition
            {
                Name = "list-files",
                Description = "List directory contents",
                InputSchema = JsonDocument.Parse("{}").RootElement
            },
            new McpToolDefinition
            {
                Name = "read-file",
                Description = "Read a file by path",
                InputSchema = JsonDocument.Parse("{}").RootElement
            },
            new McpToolDefinition
            {
                Name = "search-code",
                Description = "Search code with regex",
                InputSchema = JsonDocument.Parse("{}").RootElement
            }
        ]
    };

    [Fact]
    public void Search_NullQuery_ReturnsAll()
    {
        var svc = new McpToolSearchService();
        var results = svc.Search(CreateTestDescriptor());

        Assert.Equal(3, results.Count);
        Assert.All(results, r => Assert.NotEmpty(r.Name));
    }

    [Fact]
    public void Search_FilterQuery_NarrowsResults()
    {
        var svc = new McpToolSearchService();
        var results = svc.Search(CreateTestDescriptor(), "file");

        Assert.Equal(2, results.Count);
        Assert.Contains(results, r => r.Name == "list-files");
        Assert.Contains(results, r => r.Name == "read-file");
    }

    [Fact]
    public void Resolve_ExistingTool_ReturnsFullDefinition()
    {
        var svc = new McpToolResolveService();
        var result = svc.Resolve(CreateTestDescriptor(), "search-code");

        Assert.NotNull(result);
        Assert.Equal("search-code", result.Name);
        Assert.NotNull(result.InputSchema);
    }

    [Fact]
    public void Resolve_MissingTool_ReturnsNull()
    {
        var svc = new McpToolResolveService();
        var result = svc.Resolve(CreateTestDescriptor(), "nonexistent");

        Assert.Null(result);
    }
}
