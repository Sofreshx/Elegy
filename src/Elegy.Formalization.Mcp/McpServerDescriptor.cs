namespace Elegy.Formalization.Mcp;

public sealed record McpServerDescriptor
{
    public string ServerName { get; init; } = string.Empty;
    public McpTransportKind Transport { get; init; }
    public IReadOnlyList<McpToolDefinition> Tools { get; init; } = [];
}
