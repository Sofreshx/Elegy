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
            Description = "Portable workflow",
            SpecVersion = "v2",
            CanonicalAuthority = CanonicalAuthority.Dsl,
            ConflictPolicy = ConflictPolicy.Reconcile,
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "v2",
                PinnedRevisionId = "rev-2",
                PinnedAt = DateTimeOffset.Parse("2025-02-01T00:00:00+00:00"),
                CanonicalAuthority = CanonicalAuthority.Dsl,
                ConflictPolicy = ConflictPolicy.Reconcile
            },
            EntryStepId = "s1",
            Steps =
            [
                new WorkflowStep
                {
                    Id = "s1",
                    Name = "Start",
                    Description = "First step",
                    Type = "start",
                    ToolId = "tool.start",
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["timeoutSeconds"] = JsonSerializer.SerializeToElement(30)
                    }
                }
            ],
            Connections =
            [
                new WorkflowConnection
                {
                    Id = "c1",
                    FromStepId = "s1",
                    FromPort = "out",
                    ToStepId = "s1",
                    ToPort = "in",
                    Label = "loop",
                    Condition = "retry",
                    Priority = 1
                }
            ],
            Triggers =
            [
                new WorkflowTrigger
                {
                    Id = "t1",
                    Name = "Manual",
                    Type = "manual",
                    TargetStepId = "s1",
                    EventType = "workflow.manual",
                    WebhookSecret = "redact-me"
                }
            ],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
            {
                ["token"] = new()
                {
                    Name = "token",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("secret"),
                    IsSecret = true
                }
            },
            Layout = new WorkflowLayout
            {
                Groups = [new WorkflowGroupLayout { Id = "g1", Name = "Group", X = 1, Y = 2, Width = 3, Height = 4 }],
                Positions = [new WorkflowStepPosition { StepId = "s1", X = 5, Y = 6 }]
            },
            StrictValidation = true
        };

        var json = CanonicalJsonSerializer.Serialize(original);
        var roundTripped = CanonicalJsonSerializer.Deserialize<WorkflowDefinition>(json);

        Assert.Contains("\"description\":\"Portable workflow\"", json);
        Assert.Contains("\"canonicalAuthority\":\"dsl\"", json);
        Assert.Contains("\"conflictPolicy\":\"reconcile\"", json);

        Assert.Equal(original.Id, roundTripped.Id);
        Assert.Equal(original.Name, roundTripped.Name);
        Assert.Equal(original.Description, roundTripped.Description);
        Assert.Equal(original.SpecVersion, roundTripped.SpecVersion);
        Assert.Equal(original.CanonicalAuthority, roundTripped.CanonicalAuthority);
        Assert.Equal(original.ConflictPolicy, roundTripped.ConflictPolicy);
        Assert.Equal(original.Blueprint, roundTripped.Blueprint);
        Assert.Equal(original.EntryStepId, roundTripped.EntryStepId);
        Assert.Equal(original.Triggers.Single(), roundTripped.Triggers.Single());
        var originalStep = original.Steps.Single();
        var roundTrippedStep = roundTripped.Steps.Single();
        Assert.Equal(originalStep.Id, roundTrippedStep.Id);
        Assert.Equal(originalStep.Name, roundTrippedStep.Name);
        Assert.Equal(originalStep.Description, roundTrippedStep.Description);
        Assert.Equal(originalStep.Type, roundTrippedStep.Type);
        Assert.Equal(originalStep.ToolId, roundTrippedStep.ToolId);
        Assert.Equal(originalStep.Condition, roundTrippedStep.Condition);
        Assert.Equal(originalStep.IsEnabled, roundTrippedStep.IsEnabled);
        Assert.Equal(
            originalStep.Config["timeoutSeconds"].GetRawText(),
            roundTrippedStep.Config["timeoutSeconds"].GetRawText());
        Assert.Equal(original.Connections.Single(), roundTripped.Connections.Single());
        var originalVariable = original.Variables["token"];
        var roundTrippedVariable = roundTripped.Variables["token"];
        Assert.Equal(originalVariable.Name, roundTrippedVariable.Name);
        Assert.Equal(originalVariable.Description, roundTrippedVariable.Description);
        Assert.Equal(originalVariable.DataType, roundTrippedVariable.DataType);
        Assert.Equal(originalVariable.IsSecret, roundTrippedVariable.IsSecret);
        Assert.Equal(
            originalVariable.DefaultValue?.GetRawText(),
            roundTrippedVariable.DefaultValue?.GetRawText());
        Assert.Equal(original.Layout.Groups.Single(), roundTripped.Layout.Groups.Single());
        Assert.Equal(original.Layout.Positions.Single(), roundTripped.Layout.Positions.Single());
        Assert.True(roundTripped.StrictValidation);
    }

    [Fact]
    public void Deserialize_Rejects_Numeric_Enum_Tokens()
    {
        const string json = "{\"id\":\"wf-1\",\"name\":\"Demo\",\"specVersion\":\"v1\",\"canonicalAuthority\":1,\"conflictPolicy\":\"reconcile\",\"blueprint\":{\"specVersion\":\"v1\",\"canonicalAuthority\":\"dsl\",\"conflictPolicy\":\"reject\"},\"triggers\":[],\"steps\":[],\"connections\":[],\"variables\":{},\"layout\":{\"groups\":[],\"positions\":[]}}";

        Assert.Throws<JsonException>(() => CanonicalJsonSerializer.Deserialize<WorkflowDefinition>(json));
    }
}
