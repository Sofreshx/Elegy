using System.Text.Json;

namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowStep
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }

    public string Type { get; init; } = string.Empty;

    public string? ToolId { get; init; }

    public IReadOnlyDictionary<string, JsonElement> Config { get; init; } = new SortedDictionary<string, JsonElement>(StringComparer.Ordinal);

    public string? Condition { get; init; }

    public bool IsEnabled { get; init; } = true;
}
