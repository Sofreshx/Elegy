namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowLayout
{
    public IReadOnlyList<WorkflowGroupLayout> Groups { get; init; } = [];

    public IReadOnlyList<WorkflowStepPosition> Positions { get; init; } = [];
}
