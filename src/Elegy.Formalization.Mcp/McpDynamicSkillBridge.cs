using Elegy.Formalization.Core.Agentic;
using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Monitoring;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.Mcp;

public sealed record McpBridgeResult
{
    public IReadOnlyList<DynamicSkillCreateResult> Results { get; init; } = [];
    public IReadOnlyList<AgenticEvent> Events { get; init; } = [];
}

public sealed class McpDynamicSkillBridge
{
    public McpBridgeResult RegisterMcpSkills(DynamicSkillEngine engine, McpSkillGenerationResult generationResult)
    {
        var results = new List<DynamicSkillCreateResult>();
        var events = new List<AgenticEvent>();

        foreach (var skill in generationResult.GeneratedSkills)
        {
            DynamicSkillCreateResult createResult;
            try
            {
                var request = new DynamicSkillCreateRequest
                {
                    Name = skill.Name,
                    Description = skill.Description,
                    Triggers = skill.Triggers,
                    Constraints = skill.Constraints
                };

                createResult = engine.Create(request);
            }
            catch (InvalidOperationException ex)
            {
                createResult = new DynamicSkillCreateResult
                {
                    Success = false,
                    ErrorMessage = ex.Message
                };
            }

            results.Add(createResult);

            var mcpToolName = skill.Id.StartsWith("mcp-", StringComparison.Ordinal)
                ? skill.Id["mcp-".Length..]
                : skill.Id;

            events.Add(new AgenticEvent
            {
                EventId = Guid.NewGuid().ToString("N"),
                Timestamp = DateTimeOffset.UtcNow,
                EntityKind = AgenticEntityKind.DynamicSkill,
                EntityId = createResult.CreatedSkill?.Id ?? skill.Id,
                Category = EventCategory.Lifecycle,
                Severity = createResult.Success ? MonitoringSeverity.Info : MonitoringSeverity.Warning,
                Message = createResult.Success
                    ? $"MCP skill '{skill.Name}' registered successfully."
                    : $"MCP skill '{skill.Name}' registration failed: {createResult.ErrorMessage}",
                Metadata = new Dictionary<string, string>
                {
                    ["origin"] = "mcp-generated",
                    ["mcpToolName"] = mcpToolName
                }
            });
        }

        return new McpBridgeResult
        {
            Results = results,
            Events = events
        };
    }
}
