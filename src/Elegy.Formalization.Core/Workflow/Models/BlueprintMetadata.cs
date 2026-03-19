using Elegy.Formalization.Core.Workflow;

namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record BlueprintMetadata
{
    public const string DefaultSpecVersion = "v1";

    public string SpecVersion { get; init; } = DefaultSpecVersion;

    public string? PinnedRevisionId { get; init; }

    public DateTimeOffset? PinnedAt { get; init; }

    public CanonicalAuthority CanonicalAuthority { get; init; } = CanonicalAuthority.Dsl;

    public ConflictPolicy ConflictPolicy { get; init; } = ConflictPolicy.Reject;
}
