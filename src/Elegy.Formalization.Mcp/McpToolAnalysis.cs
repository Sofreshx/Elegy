using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp;

public sealed record McpToolAnalysis
{
    public required McpToolDefinition Tool { get; init; }
    public IReadOnlyList<SkillTrigger> ExtractedTriggers { get; init; } = [];
    public bool HasValidSchema { get; init; }
}
