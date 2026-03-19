namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowConnection
{
    public string Id { get; init; } = string.Empty;

    public string FromStepId { get; init; } = string.Empty;

    public string FromPort { get; init; } = string.Empty;

    public string ToStepId { get; init; } = string.Empty;

    public string ToPort { get; init; } = string.Empty;

    public string? Condition { get; init; }

    public string? Label { get; init; }

    public int Priority { get; init; }
}
