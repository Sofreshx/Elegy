using System.Text;
using System.Text.RegularExpressions;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp;

public sealed class McpToolAnalyzer
{
    private static readonly Regex CamelCaseBoundary = new(
        @"(?<=[a-z])(?=[A-Z])|(?<=[A-Z])(?=[A-Z][a-z])",
        RegexOptions.Compiled);

    public McpAnalysisResult Analyze(McpServerDescriptor descriptor)
    {
        var analyses = descriptor.Tools
            .Select(AnalyzeTool)
            .ToList();

        return new McpAnalysisResult
        {
            ServerName = descriptor.ServerName,
            Analyses = analyses
        };
    }

    private static McpToolAnalysis AnalyzeTool(McpToolDefinition tool)
    {
        return new McpToolAnalysis
        {
            Tool = tool,
            ExtractedTriggers = ExtractTriggers(tool.Name),
            HasValidSchema = tool.InputSchema is not null
        };
    }

    private static List<SkillTrigger> ExtractTriggers(string toolName)
    {
        if (string.IsNullOrWhiteSpace(toolName))
            return [];

        var words = SplitToolName(toolName);
        var pattern = string.Join(" ", words).ToLowerInvariant();

        return
        [
            new SkillTrigger
            {
                Pattern = pattern,
                Description = "Extracted from MCP tool name"
            }
        ];
    }

    private static string[] SplitToolName(string name)
    {
        // Split on kebab-case and snake_case delimiters first
        var parts = name.Split(['-', '_'], StringSplitOptions.RemoveEmptyEntries);

        var words = new List<string>();
        foreach (var part in parts)
        {
            // Further split camelCase boundaries
            var camelWords = CamelCaseBoundary.Split(part);
            words.AddRange(camelWords.Where(w => w.Length > 0));
        }

        return words.ToArray();
    }
}
