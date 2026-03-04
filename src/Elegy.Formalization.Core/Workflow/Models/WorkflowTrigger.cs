namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowTrigger
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string Type { get; init; } = string.Empty;

    public string? TargetStepId { get; init; }
}
