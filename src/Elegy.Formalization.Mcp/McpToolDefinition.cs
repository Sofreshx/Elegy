using System.Text.Json;

namespace Elegy.Formalization.Mcp;

public sealed record McpToolDefinition
{
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
    public JsonElement? InputSchema { get; init; }
}
