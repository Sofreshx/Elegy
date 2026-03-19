using System.Text;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp;

internal sealed record McpSkillGenerationResult
{
    public IReadOnlyList<SkillDefinition> GeneratedSkills { get; init; } = [];
    public IReadOnlyList<McpToolDefinition> SkippedTools { get; init; } = [];
}

internal sealed class McpSkillGenerator
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
            var skillId = $"mcp-{slug}";
            var sourceRef = $"mcp://{analysisResult.ServerName}/tools/{slug}";
            var skill = new SkillDefinition
            {
                Id = skillId,
                Name = analysis.Tool.Name,
                Description = analysis.Tool.Description,
                Triggers = analysis.ExtractedTriggers,
                Constraints = [],
                Identity = new SkillIdentity
                {
                    DefinitionId = skillId,
                    DisplayName = analysis.Tool.Name,
                    Namespace = analysisResult.ServerName,
                },
                Metadata = new SkillMetadata
                {
                    Summary = analysis.Tool.Description,
                    Category = "mcp",
                    Tags = BuildKeywords(analysisResult.ServerName, analysis.Tool.Name),
                },
                Input = new SkillInputContract
                {
                    SchemaRef = analysis.Tool.InputSchema is null ? null : $"{sourceRef}/input-schema",
                },
                Governance = new SkillGovernanceMetadata
                {
                    AllowedContexts = ["mcp"],
                },
                Discovery = new SkillDiscoveryMetadata
                {
                    Keywords = BuildKeywords(analysisResult.ServerName, analysis.Tool.Name),
                    CapabilityHints = analysis.ExtractedTriggers.Select(static trigger => trigger.Pattern).ToArray(),
                },
                Origin = new SkillOrigin
                {
                    MaterializationKind = SkillMaterializationKind.Declared,
                    SourceKind = SkillSourceKind.Generated,
                    SourceRef = sourceRef,
                },
                LifecycleState = SkillLifecycleState.Draft,
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

    private static string[] BuildKeywords(string serverName, string toolName)
    {
        return new[] { serverName }
            .Concat(toolName.Split(new[] { '-', '_', ' ' }, StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries))
            .Select(static keyword => keyword.ToLowerInvariant())
            .Distinct(StringComparer.OrdinalIgnoreCase)
            .ToArray();
    }
}
