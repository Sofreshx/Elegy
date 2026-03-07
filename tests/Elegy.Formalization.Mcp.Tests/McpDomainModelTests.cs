using Xunit;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpDomainModelTests
{
    [Fact]
    public void McpServerDescriptor_default_construction_succeeds()
    {
        var descriptor = new McpServerDescriptor
        {
            ServerName = "test-server",
            Transport = McpTransportKind.Stdio,
            Tools = [new McpToolDefinition { Name = "tool-a", Description = "A tool" }]
        };

        Assert.Equal("test-server", descriptor.ServerName);
        Assert.Single(descriptor.Tools);
    }
}
