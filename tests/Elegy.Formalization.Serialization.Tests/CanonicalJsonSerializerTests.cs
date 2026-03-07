using Elegy.Formalization.Core.Workflow;
using Elegy.Formalization.Core.Workflow.Models;
using Elegy.Formalization.Serialization;
using System.Text.Json;
using Xunit;

namespace Elegy.Formalization.Serialization.Tests;

public sealed class CanonicalJsonSerializerTests
{
    [Fact]
    public void Serialize_And_Deserialize_WorkflowDefinition_RoundTrips()
    {
        var original = new WorkflowDefinition
        {
            Id = "wf-1",
            Name = "Demo",
            SpecVersion = "1.0",
            CanonicalAuthority = CanonicalAuthority.Blueprint,
            ConflictPolicy = ConflictPolicy.Reconcile,
            Blueprint = new BlueprintMetadata
            {
                BlueprintId = "bp-1",
                Version = "2",
                IsPinned = true
            },
            Steps = [new WorkflowStep { Id = "s1", Name = "Start", Type = "start" }],
            Connections = [],
            Triggers = [new WorkflowTrigger { Id = "t1", Name = "Manual", Type = "manual", TargetStepId = "s1" }],
            Layout = new WorkflowLayout
            {
                Groups = [new WorkflowGroupLayout { Id = "g1", Name = "Group", X = 1, Y = 2, Width = 3, Height = 4 }],
                Positions = [new WorkflowStepPosition { StepId = "s1", X = 5, Y = 6 }]
            }
        };

        var json = CanonicalJsonSerializer.Serialize(original);
        var roundTripped = CanonicalJsonSerializer.Deserialize<WorkflowDefinition>(json);

        Assert.Contains("\"canonicalAuthority\":\"blueprint\"", json);
        Assert.Contains("\"conflictPolicy\":\"reconcile\"", json);

        Assert.Equal(original.Id, roundTripped.Id);
        Assert.Equal(original.Name, roundTripped.Name);
        Assert.Equal(original.SpecVersion, roundTripped.SpecVersion);
        Assert.Equal(original.CanonicalAuthority, roundTripped.CanonicalAuthority);
        Assert.Equal(original.ConflictPolicy, roundTripped.ConflictPolicy);
        Assert.Equal(original.Blueprint, roundTripped.Blueprint);
        Assert.Equal(original.Triggers.Single(), roundTripped.Triggers.Single());
        Assert.Equal(original.Steps.Single(), roundTripped.Steps.Single());
        Assert.Equal(original.Layout.Groups.Single(), roundTripped.Layout.Groups.Single());
        Assert.Equal(original.Layout.Positions.Single(), roundTripped.Layout.Positions.Single());
    }

    [Fact]
    public void Deserialize_Rejects_Numeric_Enum_Tokens()
    {
        const string json = "{\"id\":\"wf-1\",\"name\":\"Demo\",\"specVersion\":\"1.0\",\"canonicalAuthority\":1,\"conflictPolicy\":\"reconcile\",\"blueprint\":{\"blueprintId\":\"bp-1\",\"version\":\"2\",\"isPinned\":true},\"triggers\":[],\"steps\":[],\"connections\":[],\"layout\":{\"groups\":[],\"positions\":[]}}";

        Assert.Throws<JsonException>(() => CanonicalJsonSerializer.Deserialize<WorkflowDefinition>(json));
    }
}
