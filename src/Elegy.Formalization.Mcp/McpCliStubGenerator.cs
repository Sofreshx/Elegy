using System.Text.Json;

namespace Elegy.Formalization.Mcp;

public sealed record McpCliArgument
{
    public string Name { get; init; } = string.Empty;
    public string Type { get; init; } = "string";
    public bool Required { get; init; }
    public string? Description { get; init; }
}

public sealed record McpCliStubDefinition
{
    public string CommandName { get; init; } = string.Empty;
    public string? Description { get; init; }
    public IReadOnlyList<McpCliArgument> Arguments { get; init; } = [];
    public required McpToolDefinition SourceTool { get; init; }
}

public sealed class McpCliStubGenerator
{
    public IReadOnlyList<McpCliStubDefinition> Generate(McpAnalysisResult analysisResult)
    {
        var stubs = new List<McpCliStubDefinition>();

        foreach (var analysis in analysisResult.Analyses)
        {
            var arguments = ExtractArguments(analysis.Tool.InputSchema);
            var commandName = analysis.Tool.Name.Replace('_', '-').ToLowerInvariant();

            stubs.Add(new McpCliStubDefinition
            {
                CommandName = commandName,
                Description = analysis.Tool.Description,
                Arguments = arguments,
                SourceTool = analysis.Tool
            });
        }

        return stubs;
    }

    private static List<McpCliArgument> ExtractArguments(JsonElement? inputSchema)
    {
        if (inputSchema is null)
            return [];

        var schema = inputSchema.Value;
        if (schema.ValueKind != JsonValueKind.Object)
            return [];

        if (!schema.TryGetProperty("properties", out var properties))
            return [];

        var requiredSet = new HashSet<string>(StringComparer.Ordinal);
        if (schema.TryGetProperty("required", out var requiredArray) &&
            requiredArray.ValueKind == JsonValueKind.Array)
        {
            foreach (var item in requiredArray.EnumerateArray())
            {
                if (item.GetString() is { } s)
                    requiredSet.Add(s);
            }
        }

        var args = new List<McpCliArgument>();
        foreach (var prop in properties.EnumerateObject())
        {
            var type = "string";
            if (prop.Value.TryGetProperty("type", out var typeEl))
                type = typeEl.GetString() ?? "string";

            string? desc = null;
            if (prop.Value.TryGetProperty("description", out var descEl))
                desc = descEl.GetString();

            args.Add(new McpCliArgument
            {
                Name = prop.Name,
                Type = type,
                Required = requiredSet.Contains(prop.Name),
                Description = desc
            });
        }

        return args;
    }
}
