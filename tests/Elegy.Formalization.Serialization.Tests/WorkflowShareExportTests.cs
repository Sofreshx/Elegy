using System.Text.Json;
using Elegy.Formalization.Core.Workflow;
using Elegy.Formalization.Core.Workflow.Models;
using Xunit;

namespace Elegy.Formalization.Serialization.Tests;

public sealed class WorkflowShareExportTests
{
    [Fact]
    public void FromDefinition_Redacts_Secrets_And_Adds_Representation_Metadata()
    {
        var workflow = new WorkflowDefinition
        {
            Id = "wf-1",
            Name = "Shared Workflow",
            Description = "safe for export",
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "  ",
                PinnedRevisionId = "rev-7",
                CanonicalAuthority = CanonicalAuthority.Dsl,
                ConflictPolicy = ConflictPolicy.Override
            },
            Triggers =
            [
                new WorkflowTrigger
                {
                    Id = "t-1",
                    Name = "Webhook",
                    Type = "webhook",
                    WebhookSecret = "top-secret",
                    TargetStepId = "s-1"
                }
            ],
            EntryStepId = "s-1",
            Steps =
            [
                new WorkflowStep
                {
                    Id = "s-1",
                    Name = "Start",
                    Type = "task",
                    ToolId = "tool.start",
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["retry"] = JsonSerializer.SerializeToElement(2)
                    }
                }
            ],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
            {
                ["apiKey"] = new()
                {
                    Name = "apiKey",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("secret-value"),
                    IsSecret = true
                },
                ["region"] = new()
                {
                    Name = "region",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("us-east-1")
                }
            },
            StrictValidation = true
        };

        var export = WorkflowShareExport.FromDefinition(workflow);

        Assert.Equal("workflow-share-export", export.Representation.CanonicalFormat);
        Assert.Equal(1, export.Representation.CanonicalVersion);
        Assert.Equal(["mermaid"], export.Representation.ProjectionFormats);
        Assert.Equal("v1", export.Blueprint.SpecVersion);
        Assert.Equal("rev-7", export.Blueprint.PinnedRevisionId);
        Assert.Single(export.Triggers);
        Assert.Null(export.Triggers.Single().ToTrigger().WebhookSecret);
        Assert.True(export.Variables["apiKey"].IsSecret);
        Assert.Null(export.Variables["apiKey"].DefaultValue);
        Assert.Equal("us-east-1", export.Variables["region"].DefaultValue?.GetString());
    }

    [Fact]
    public void ToDefinition_Rehydrates_Shareable_Content_Without_Reintroducing_Secrets()
    {
        var export = new WorkflowShareExport
        {
            Name = "Imported Workflow",
            Description = "portable",
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "v2",
                PinnedRevisionId = "rev-9",
                CanonicalAuthority = CanonicalAuthority.Dsl,
                ConflictPolicy = ConflictPolicy.Reconcile
            },
            Triggers =
            [
                new WorkflowTriggerExport
                {
                    Id = "t-1",
                    Name = "Webhook",
                    Type = "webhook",
                    TargetStepId = "s-1"
                }
            ],
            EntryStepId = "s-1",
            Steps =
            [
                new WorkflowStep { Id = "s-1", Name = "Start", Type = "task" }
            ],
            Variables = new Dictionary<string, WorkflowVariableExport>(StringComparer.Ordinal)
            {
                ["apiKey"] = new()
                {
                    Name = "apiKey",
                    DataType = "string",
                    IsSecret = true
                }
            },
            StrictValidation = true
        };

        var workflow = export.ToDefinition("wf-imported");

        Assert.Equal("wf-imported", workflow.Id);
        Assert.Equal("Imported Workflow", workflow.Name);
        Assert.Equal("portable", workflow.Description);
        Assert.Equal("v2", workflow.SpecVersion);
        Assert.Equal(CanonicalAuthority.Dsl, workflow.CanonicalAuthority);
        Assert.Equal(ConflictPolicy.Reconcile, workflow.ConflictPolicy);
        Assert.Equal("rev-9", workflow.Blueprint.PinnedRevisionId);
        Assert.Equal("s-1", workflow.EntryStepId);
        Assert.True(workflow.StrictValidation);
        Assert.Single(workflow.Triggers);
        Assert.Null(workflow.Triggers.Single().WebhookSecret);
        Assert.False(workflow.Variables["apiKey"].IsSecret);
        Assert.Null(workflow.Variables["apiKey"].DefaultValue);
    }
}
