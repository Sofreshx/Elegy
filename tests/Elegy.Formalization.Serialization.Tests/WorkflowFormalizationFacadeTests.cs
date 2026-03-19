using System.Text.Json;
using Elegy.Formalization.Core.Workflow;
using Elegy.Formalization.Core.Workflow.Models;
using Xunit;

namespace Elegy.Formalization.Serialization.Tests;

public sealed class WorkflowFormalizationFacadeTests
{
    private readonly IWorkflowFormalizationFacade _sut = new WorkflowFormalizationFacade();

    [Fact]
    public void NormalizePortableWorkflow_Produces_Shareable_Workflow_Surface()
    {
        var workflow = new WorkflowDefinition
        {
            Id = "wf-123",
            Name = "Shareable Workflow",
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = " ",
                CanonicalAuthority = CanonicalAuthority.Dsl,
                ConflictPolicy = ConflictPolicy.Reconcile
            },
            Triggers =
            [
                new WorkflowTrigger
                {
                    Id = "trigger-1",
                    Name = "Webhook",
                    Type = "webhook",
                    TargetStepId = "step-1",
                    WebhookSecret = "hidden"
                }
            ],
            Steps =
            [
                new WorkflowStep
                {
                    Id = "step-1",
                    Name = "Start",
                    Type = "task",
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["retries"] = JsonSerializer.SerializeToElement(3)
                    }
                }
            ],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
            {
                ["apiKey"] = new()
                {
                    Name = "apiKey",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("secret"),
                    IsSecret = true
                }
            }
        };

        var portable = _sut.NormalizePortableWorkflow(workflow);

        Assert.Equal("Shareable Workflow", portable.Name);
        Assert.Equal("v1", portable.Blueprint.SpecVersion);
        Assert.Equal("workflow-share-export", portable.Representation.CanonicalFormat);
        Assert.Null(portable.Triggers.Single().ToTrigger().WebhookSecret);
        Assert.Null(portable.Variables["apiKey"].DefaultValue);
    }

    [Fact]
    public void Serialize_And_Deserialize_PortableWorkflow_RoundTrip_And_Restore_Definition()
    {
        var portable = new WorkflowShareExport
        {
            Name = "Portable Workflow",
            Representation = new WorkflowRepresentationMetadata
            {
                CanonicalFormat = WorkflowShareExport.DefaultCanonicalFormat,
                CanonicalVersion = 1,
                ProjectionFormats = [WorkflowShareExport.MermaidProjectionFormat]
            },
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "v2",
                CanonicalAuthority = CanonicalAuthority.Dsl,
                ConflictPolicy = ConflictPolicy.Override
            },
            Steps =
            [
                new WorkflowStep
                {
                    Id = "step-1",
                    Name = "Start",
                    Type = "task"
                }
            ]
        };

        var json = _sut.SerializePortableWorkflow(portable);
        var roundTripped = _sut.DeserializePortableWorkflow(json);
        var restored = _sut.DeserializePortableWorkflowAsDefinition(json, "wf-restored");

        Assert.Contains("\"canonicalFormat\":\"workflow-share-export\"", json);
        Assert.Equal("Portable Workflow", roundTripped.Name);
        Assert.Equal("v2", roundTripped.Blueprint.SpecVersion);
        Assert.Equal("wf-restored", restored.Id);
        Assert.Equal("Portable Workflow", restored.Name);
        Assert.Equal(CanonicalAuthority.Dsl, restored.CanonicalAuthority);
        Assert.Equal(ConflictPolicy.Override, restored.ConflictPolicy);
    }
}
