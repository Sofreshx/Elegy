using Elegy.Formalization.Core.Workflow;
using Elegy.Formalization.Core.Workflow.Models;
using Xunit;

namespace Elegy.Formalization.Governance.Tests;

public sealed class WorkflowGovernanceProjectionTests
{
    [Fact]
    public void ProjectPortable_Normalizes_Blank_Blueprint_Governance_Defaults()
    {
        var workflow = new WorkflowDefinition
        {
            Id = "wf-1",
            Name = "governed",
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "   "
            }
        };

        var projected = WorkflowGovernanceProjection.ProjectPortable(workflow);

        Assert.Equal("v1", projected.Blueprint.SpecVersion);
        Assert.Equal(CanonicalAuthority.Dsl, projected.Blueprint.CanonicalAuthority);
        Assert.Equal(ConflictPolicy.Reject, projected.Blueprint.ConflictPolicy);
        Assert.Equal("v1", projected.SpecVersion);
        Assert.Equal(CanonicalAuthority.Dsl, projected.CanonicalAuthority);
        Assert.Equal(ConflictPolicy.Reject, projected.ConflictPolicy);
    }

    [Fact]
    public void ProjectPortable_Preserves_Explicit_Governance_Metadata()
    {
        var pinnedAt = DateTimeOffset.Parse("2025-05-02T12:30:00+00:00");
        var workflow = new WorkflowDefinition
        {
            Id = "wf-1",
            Name = "governed",
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "v2",
                PinnedRevisionId = "rev-12",
                PinnedAt = pinnedAt,
                CanonicalAuthority = CanonicalAuthority.Dsl,
                ConflictPolicy = ConflictPolicy.Override
            }
        };

        var projected = WorkflowGovernanceProjection.ProjectPortable(workflow);

        Assert.Equal("v2", projected.Blueprint.SpecVersion);
        Assert.Equal("rev-12", projected.Blueprint.PinnedRevisionId);
        Assert.Equal(pinnedAt, projected.Blueprint.PinnedAt);
        Assert.Equal(CanonicalAuthority.Dsl, projected.Blueprint.CanonicalAuthority);
        Assert.Equal(ConflictPolicy.Override, projected.Blueprint.ConflictPolicy);
        Assert.Equal("v2", projected.SpecVersion);
        Assert.Equal(ConflictPolicy.Override, projected.ConflictPolicy);
    }
}
