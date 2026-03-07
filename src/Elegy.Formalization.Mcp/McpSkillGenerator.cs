using System.Text;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp;

public sealed record McpSkillGenerationResult
{
    public IReadOnlyList<SkillDefinition> GeneratedSkills { get; init; } = [];
    public IReadOnlyList<McpToolDefinition> SkippedTools { get; init; } = [];
}

public sealed class McpSkillGenerator
{
    public McpSkillGenerationResult Generate(McpAnalysisResult analysisResult)
    {
        var generated = new List<SkillDefinition>();
        var skipped = new List<McpToolDefinition>();

        foreach (var analysis in analysisResult.Analyses)
        {
            if (!analysis.HasValidSchema)
            {
                skipped.Add(analysis.Tool);
                continue;
            }

            var slug = BuildSlug(analysisResult.ServerName, analysis.Tool.Name);
            var skill = new SkillDefinition
            {
                Id = $"mcp-{slug}",
                Name = analysis.Tool.Name,
                Description = analysis.Tool.Description,
                Triggers = analysis.ExtractedTriggers,
                Constraints =
                [
                    new SkillConstraint
                    {
                        ConstraintId = "origin",
                        Description = "mcp-generated",
                        Required = true
                    }
                ],
                LifecycleState = SkillLifecycleState.Draft
            };

            generated.Add(skill);
        }

        return new McpSkillGenerationResult
        {
            GeneratedSkills = generated,
            SkippedTools = skipped
        };
    }

    private static string BuildSlug(string serverName, string toolName)
    {
        var combined = $"{serverName}-{toolName}";
        var sb = new StringBuilder();
        foreach (var c in combined)
        {
            if (char.IsLetterOrDigit(c))
                sb.Append(char.ToLowerInvariant(c));
            else if (c is '-' or '_')
                sb.Append('-');
        }
        return sb.ToString().Trim('-');
    }
}
