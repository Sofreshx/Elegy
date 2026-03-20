namespace Elegy.Formalization.Mcp;

[Obsolete("Compatibility surface only. Prefer Rust runtime and CLI surfaces for executable MCP behavior.")]
public interface IMcpFormalizationFacade
{
    McpAnalysisResult Analyze(McpServerDescriptor descriptor);

    McpSkillProjectionResult ProjectSkills(McpAnalysisResult analysisResult);

    McpSkillProjectionResult ProjectSkills(McpServerDescriptor descriptor);

    IReadOnlyList<McpToolSearchResult> SearchTools(McpServerDescriptor descriptor, string? query = null);

    McpToolDefinition? ResolveTool(McpServerDescriptor descriptor, string toolName);
}
