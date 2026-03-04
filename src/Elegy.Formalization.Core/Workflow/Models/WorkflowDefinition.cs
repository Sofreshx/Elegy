using Elegy.Formalization.Core.Workflow;

namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowDefinition
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string SpecVersion { get; init; } = "1.0";

    public CanonicalAuthority CanonicalAuthority { get; init; } = CanonicalAuthority.Blueprint;

    public ConflictPolicy ConflictPolicy { get; init; } = ConflictPolicy.Reconcile;

    public BlueprintMetadata Blueprint { get; init; } = new();

    public IReadOnlyList<WorkflowTrigger> Triggers { get; init; } = [];

    public IReadOnlyList<WorkflowStep> Steps { get; init; } = [];

    public IReadOnlyList<WorkflowConnection> Connections { get; init; } = [];

    public WorkflowLayout Layout { get; init; } = new();
}
