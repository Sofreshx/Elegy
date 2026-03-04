namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowGroupLayout
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public double X { get; init; }

    public double Y { get; init; }

    public double Width { get; init; }

    public double Height { get; init; }
}
