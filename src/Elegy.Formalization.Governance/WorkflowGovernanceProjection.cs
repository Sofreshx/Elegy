using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Governance;

public static class WorkflowGovernanceProjection
{
    public static WorkflowDefinition ProjectPortable(WorkflowDefinition workflow)
    {
        ArgumentNullException.ThrowIfNull(workflow);

        var blueprint = Normalize(workflow.Blueprint);
        return workflow with
        {
            Blueprint = blueprint,
            SpecVersion = blueprint.SpecVersion,
            CanonicalAuthority = blueprint.CanonicalAuthority,
            ConflictPolicy = blueprint.ConflictPolicy
        };
    }

    public static BlueprintMetadata Normalize(BlueprintMetadata? blueprint)
    {
        var source = blueprint ?? new BlueprintMetadata();
        var specVersion = string.IsNullOrWhiteSpace(source.SpecVersion)
            ? BlueprintMetadata.DefaultSpecVersion
            : source.SpecVersion.Trim();

        return source with
        {
            SpecVersion = specVersion
        };
    }
}
