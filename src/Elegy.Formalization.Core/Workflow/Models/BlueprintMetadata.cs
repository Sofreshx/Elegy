namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record BlueprintMetadata
{
    public string BlueprintId { get; init; } = string.Empty;

    public string Version { get; init; } = string.Empty;

    public bool IsPinned { get; init; }
}
