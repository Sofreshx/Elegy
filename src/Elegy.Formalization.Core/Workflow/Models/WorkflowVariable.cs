using System.Text.Json;

namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowVariable
{
    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }

    public string DataType { get; init; } = "any";

    public JsonElement? DefaultValue { get; init; }

    public bool IsSecret { get; init; }
}
