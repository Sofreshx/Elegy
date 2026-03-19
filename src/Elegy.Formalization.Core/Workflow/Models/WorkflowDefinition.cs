using Elegy.Formalization.Core.Workflow;

namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowDefinition
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }

    public string SpecVersion { get; init; } = BlueprintMetadata.DefaultSpecVersion;

    public CanonicalAuthority CanonicalAuthority { get; init; } = CanonicalAuthority.Dsl;

    public ConflictPolicy ConflictPolicy { get; init; } = ConflictPolicy.Reject;

    public BlueprintMetadata Blueprint { get; init; } = new();

    public IReadOnlyList<WorkflowTrigger> Triggers { get; init; } = [];

    public string? EntryStepId { get; init; }

    public IReadOnlyList<WorkflowStep> Steps { get; init; } = [];

    public IReadOnlyList<WorkflowConnection> Connections { get; init; } = [];

    public IReadOnlyDictionary<string, WorkflowVariable> Variables { get; init; } = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal);

    public WorkflowLayout Layout { get; init; } = new();

    public bool StrictValidation { get; init; }
}
