namespace Elegy.Formalization.Mcp;

public interface IMcpFormalizationFacade
{
    McpAnalysisResult Analyze(McpServerDescriptor descriptor);

    McpSkillProjectionResult ProjectSkills(McpAnalysisResult analysisResult);

    McpSkillProjectionResult ProjectSkills(McpServerDescriptor descriptor);

    IReadOnlyList<McpToolSearchResult> SearchTools(McpServerDescriptor descriptor, string? query = null);

    McpToolDefinition? ResolveTool(McpServerDescriptor descriptor, string toolName);
}
