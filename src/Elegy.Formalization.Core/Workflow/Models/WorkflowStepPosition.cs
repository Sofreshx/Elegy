namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowStepPosition
{
    public string StepId { get; init; } = string.Empty;

    public double X { get; init; }

    public double Y { get; init; }
}
