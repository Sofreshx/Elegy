using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp;

[Obsolete("Compatibility surface only. Prefer Rust runtime and CLI surfaces for executable MCP behavior.")]
public sealed record McpSkillProjectionResult
{
    public IReadOnlyList<SkillDefinition> GeneratedSkills { get; init; } = [];

    public IReadOnlyList<McpToolDefinition> SkippedTools { get; init; } = [];
}

[Obsolete("Compatibility surface only. Prefer Rust runtime and CLI surfaces for executable MCP behavior.")]
public sealed record McpToolSearchResult
{
    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }
}

[Obsolete("Compatibility surface only. Prefer Rust runtime and CLI surfaces for executable MCP behavior.")]
public sealed class McpFormalizationFacade : IMcpFormalizationFacade
{
    private readonly McpToolAnalyzer _analyzer = new();
    private readonly McpSkillGenerator _skillGenerator = new();
    private readonly McpToolSearchService _searchService = new();
    private readonly McpToolResolveService _resolveService = new();

    public McpAnalysisResult Analyze(McpServerDescriptor descriptor)
    {
        ArgumentNullException.ThrowIfNull(descriptor);
        return _analyzer.Analyze(descriptor);
    }

    public McpSkillProjectionResult ProjectSkills(McpAnalysisResult analysisResult)
    {
        ArgumentNullException.ThrowIfNull(analysisResult);

        var projection = _skillGenerator.Generate(analysisResult);
        return new McpSkillProjectionResult
        {
            GeneratedSkills = projection.GeneratedSkills,
            SkippedTools = projection.SkippedTools
        };
    }

    public McpSkillProjectionResult ProjectSkills(McpServerDescriptor descriptor)
    {
        return ProjectSkills(Analyze(descriptor));
    }

    public IReadOnlyList<McpToolSearchResult> SearchTools(McpServerDescriptor descriptor, string? query = null)
    {
        ArgumentNullException.ThrowIfNull(descriptor);

        return _searchService.Search(descriptor, query)
            .Select(static result => new McpToolSearchResult
            {
                Name = result.Name,
                Description = result.Description
            })
            .ToArray();
    }

    public McpToolDefinition? ResolveTool(McpServerDescriptor descriptor, string toolName)
    {
        ArgumentNullException.ThrowIfNull(descriptor);
        ArgumentException.ThrowIfNullOrWhiteSpace(toolName);
        return _resolveService.Resolve(descriptor, toolName);
    }
}
