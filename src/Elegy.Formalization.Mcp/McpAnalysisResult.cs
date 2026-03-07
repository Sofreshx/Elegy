namespace Elegy.Formalization.Mcp;

public sealed record McpAnalysisResult
{
    public string ServerName { get; init; } = string.Empty;
    public IReadOnlyList<McpToolAnalysis> Analyses { get; init; } = [];
}
