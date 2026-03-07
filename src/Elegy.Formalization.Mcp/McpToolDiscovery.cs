namespace Elegy.Formalization.Mcp;

public sealed record McpToolSummary
{
    public string Name { get; init; } = string.Empty;
    public string? Description { get; init; }
}

public sealed class McpToolSearchService
{
    public IReadOnlyList<McpToolSummary> Search(McpServerDescriptor descriptor, string? query = null)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return descriptor.Tools
                .Select(t => new McpToolSummary { Name = t.Name, Description = t.Description })
                .ToList();
        }

        return descriptor.Tools
            .Where(t =>
                t.Name.Contains(query, StringComparison.OrdinalIgnoreCase) ||
                (t.Description is not null && t.Description.Contains(query, StringComparison.OrdinalIgnoreCase)))
            .Select(t => new McpToolSummary { Name = t.Name, Description = t.Description })
            .ToList();
    }
}

public sealed class McpToolResolveService
{
    public McpToolDefinition? Resolve(McpServerDescriptor descriptor, string toolName)
    {
        return descriptor.Tools
            .FirstOrDefault(t => t.Name.Equals(toolName, StringComparison.Ordinal));
    }
}
