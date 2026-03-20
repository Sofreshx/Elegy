using System.Text.Json;
using Elegy.Formalization.Core.Workflow;
using Elegy.Formalization.Core.Workflow.Models;
using Xunit;

namespace Elegy.Formalization.Serialization.Tests;

public sealed class CanonicalWorkflowGraphFacadeTests
{
    private readonly IWorkflowFormalizationFacade _sut = new WorkflowFormalizationFacade();

    [Fact]
    public void SerializeCanonicalWorkflowGraph_IsDeterministic_ForEquivalentGraphs()
    {
        var first = BuildCanonicalGraph();
        var second = first with
        {
            Nodes = [first.Nodes[1], first.Nodes[0]],
            Edges = [first.Edges[1], first.Edges[0]],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
            {
                ["zeta"] = new()
                {
                    Name = "zeta",
                    DataType = "string"
                },
                ["alpha"] = new()
                {
                    Name = "alpha",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("A-123")
                }
            }
        };

        var jsonA = _sut.SerializeCanonicalWorkflowGraph(first);
        var jsonB = _sut.SerializeCanonicalWorkflowGraph(second);
        var roundTripped = _sut.DeserializeCanonicalWorkflowGraph(jsonA);

        Assert.Equal(jsonA, jsonB);
        Assert.Equal(["alpha", "zeta"], roundTripped.Variables.Keys.ToArray());
        Assert.Equal(["step-a", "step-b"], roundTripped.Nodes.Select(static node => node.Id).ToArray());
        Assert.NotNull(roundTripped.Trigger);
        Assert.Equal(["approval", "result"], roundTripped.Trigger!.InputSchema.Select(static port => port.Name).ToArray());

        var normalizedNode = roundTripped.Nodes.Single(static node => node.Id == "step-b");
        Assert.Equal(["alpha", "zeta"], normalizedNode.Config.Keys.ToArray());
        Assert.Equal(["alpha", "zeta"], normalizedNode.InputMappings.Keys.ToArray());
        Assert.Equal(["alpha", "zeta"], normalizedNode.InputResolutions.Keys.ToArray());
        Assert.Equal(["ops", "reviewers"], normalizedNode.HumanReview?.ApproverRoles);
        Assert.Equal(["anna", "zoe"], normalizedNode.HumanReview?.ApproverUserIds);
        Assert.Equal(["fatal", "transient"], normalizedNode.RetryConfig?.RetryableErrorCodes);
        Assert.Equal(["alpha", "zeta"], normalizedNode.Inputs.Select(static input => input.Name).ToArray());
    }

    [Fact]
    public void DeserializeCanonicalWorkflowGraph_AllowsCommentsAndTrailingCommas()
    {
        const string json = """
            {
              // canonical graph fixture
              "canonicalFormat": "canonical-workflow-graph",
              "canonicalVersion": 1,
              "trigger": {
                "type": "event",
                "eventType": "contract.updated",
              },
              "entryStepId": "step-a",
              "nodes": [
                {
                  "id": "step-a",
                  "name": "Start",
                  "type": "task",
                  "isEnabled": true,
                },
              ],
              "edges": [
                {
                  "fromStepId": "step-a",
                  "fromPort": "result",
                  "toStepId": "step-b",
                  "toPort": "input",
                  "priority": 10,
                },
              ],
              "variables": {
                "tenant": {
                  "name": "tenant",
                  "dataType": "string",
                },
              },
            }
            """;

        var graph = _sut.DeserializeCanonicalWorkflowGraph(json);
        var normalizedJson = _sut.SerializeCanonicalWorkflowGraph(graph);

        Assert.Equal("canonical-workflow-graph", graph.CanonicalFormat);
        Assert.Equal("contract.updated", graph.Trigger.EventType);
        Assert.Equal(["step-a"], graph.Nodes.Select(static node => node.Id).ToArray());
        Assert.Contains("\"canonicalFormat\":\"canonical-workflow-graph\"", normalizedJson);
        Assert.DoesNotContain("// canonical graph fixture", normalizedJson, StringComparison.Ordinal);
        Assert.Contains("\"variables\":{\"tenant\":", normalizedJson, StringComparison.Ordinal);
    }

    [Fact]
    public void NormalizeCanonicalWorkflowGraph_FromPortableWorkflow_AndApplyBack_PreservesGraphFields()
    {
        var workflow = new WorkflowDefinition
        {
            Id = "wf-123",
            Name = "Canonical Graph Source",
            Description = "Portable source workflow",
            SpecVersion = "v4",
            CanonicalAuthority = CanonicalAuthority.Runtime,
            ConflictPolicy = ConflictPolicy.Override,
            Blueprint = new BlueprintMetadata
            {
                SpecVersion = "v4",
                PinnedRevisionId = "rev-9",
                PinnedAt = DateTimeOffset.Parse("2026-03-20T12:34:56+00:00"),
                CanonicalAuthority = CanonicalAuthority.Runtime,
                ConflictPolicy = ConflictPolicy.Override
            },
            Triggers =
            [
                new WorkflowTrigger
                {
                    Id = "trigger-1",
                    Name = "Trigger",
                    Type = "event",
                    EventType = "contract.updated",
                    Timezone = "America/New_York"
                }
            ],
            EntryStepId = "step-b",
            Steps =
            [
                new WorkflowStep
                {
                    Id = "step-b",
                    Name = "Approve",
                    Description = "Decision step",
                    Type = "approval",
                    ToolId = "tool.approve",
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["zeta"] = JsonSerializer.SerializeToElement(true),
                        ["alpha"] = JsonSerializer.SerializeToElement("strict")
                    },
                    Condition = "score >= 80",
                    IsEnabled = false
                },
                new WorkflowStep
                {
                    Id = "step-a",
                    Name = "Collect",
                    Type = "task",
                    ToolId = "tool.collect"
                }
            ],
            Connections =
            [
                new WorkflowConnection
                {
                    Id = "c-1",
                    FromStepId = "step-a",
                    FromPort = "result",
                    ToStepId = "step-b",
                    ToPort = "input",
                    Label = "next",
                    Priority = 2
                }
            ],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
            {
                ["tenant"] = new()
                {
                    Name = "tenant",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("tenant-a")
                },
                ["apiKey"] = new()
                {
                    Name = "apiKey",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("secret"),
                    IsSecret = true
                }
            },
            Layout = new WorkflowLayout
            {
                Groups =
                [
                    new WorkflowGroupLayout
                    {
                        Id = "group-1",
                        Name = "Preserve Layout",
                        X = 1,
                        Y = 2,
                        Width = 3,
                        Height = 4
                    }
                ]
            },
            StrictValidation = true
        };

        var graph = _sut.NormalizeCanonicalWorkflowGraph(workflow);
        var template = workflow with
        {
            Id = "wf-template",
            Name = "Template Metadata",
            Description = "Should be preserved",
            StrictValidation = false,
            Triggers = [],
            Steps = [],
            Connections = [],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
        };

        var restored = _sut.ApplyCanonicalWorkflowGraph(template, graph);

        Assert.Null(graph.Variables["apiKey"].DefaultValue);
        Assert.Equal("wf-template", restored.Id);
        Assert.Equal("Template Metadata", restored.Name);
        Assert.Equal("Should be preserved", restored.Description);
        Assert.False(restored.StrictValidation);
        Assert.Equal(CanonicalAuthority.Runtime, restored.CanonicalAuthority);
        Assert.Equal(ConflictPolicy.Override, restored.ConflictPolicy);
        Assert.Single(restored.Layout.Groups);
        Assert.Equal("step-b", restored.EntryStepId);
        Assert.Equal("event", restored.Triggers.Single().Type);
        Assert.Equal("contract.updated", restored.Triggers.Single().EventType);
        Assert.Equal(["step-a", "step-b"], restored.Steps.Select(static step => step.Id).ToArray());
        Assert.Equal("strict", restored.Steps.Single(static step => step.Id == "step-b").Config["alpha"].GetString());
        Assert.Single(restored.Connections);
        Assert.Equal("tenant-a", restored.Variables["tenant"].DefaultValue?.GetString());
        Assert.Null(restored.Variables["apiKey"].DefaultValue);
    }

    [Fact]
    public void NormalizeCanonicalWorkflowGraph_Rejects_MultiplePortableTriggers()
    {
        var workflow = new WorkflowDefinition
        {
            Id = "wf-123",
            Name = "Multi Trigger",
            Triggers =
            [
                new WorkflowTrigger { Id = "trigger-1", Name = "A", Type = "manual" },
                new WorkflowTrigger { Id = "trigger-2", Name = "B", Type = "event" }
            ]
        };

        Assert.Throws<InvalidOperationException>(() => _sut.NormalizeCanonicalWorkflowGraph(workflow));
    }

    [Fact]
    public void SerializeCanonicalWorkflowGraph_NormalizesNestedJsonObjects()
    {
        var first = new CanonicalWorkflowGraph
        {
            Nodes =
            [
                new CanonicalWorkflowNode
                {
                    Id = "step-a",
                    Name = "Nested",
                    Type = "task",
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["payload"] = JsonDocument.Parse("""{"beta":2,"alpha":1}""").RootElement.Clone()
                    }
                }
            ]
        };
        var second = new CanonicalWorkflowGraph
        {
            Nodes =
            [
                new CanonicalWorkflowNode
                {
                    Id = "step-a",
                    Name = "Nested",
                    Type = "task",
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["payload"] = JsonDocument.Parse("""{"alpha":1,"beta":2}""").RootElement.Clone()
                    }
                }
            ]
        };

        var jsonA = _sut.SerializeCanonicalWorkflowGraph(first);
        var jsonB = _sut.SerializeCanonicalWorkflowGraph(second);

        Assert.Equal(jsonA, jsonB);
    }

    [Fact]
    public void ApplyCanonicalWorkflowGraph_RejectsFieldsNotRepresentableByPortableWorkflow()
    {
        var template = new WorkflowDefinition
        {
            Id = "wf-template",
            Name = "Template"
        };
        var graph = new CanonicalWorkflowGraph
        {
            Trigger = new CanonicalWorkflowTrigger
            {
                Type = "event",
                InputSchema =
                [
                    new CanonicalPortDefinition
                    {
                        Name = "payload",
                        DataType = "object"
                    }
                ]
            },
            Nodes =
            [
                new CanonicalWorkflowNode
                {
                    Id = "step-a",
                    Name = "Rich Step",
                    Type = "task",
                    PieceId = "piece-1",
                    PieceType = "addon",
                    AddonVersion = 2,
                    Inputs =
                    [
                        new CanonicalPortDefinition
                        {
                            Name = "input",
                            DataType = "string"
                        }
                    ],
                    Outputs =
                    [
                        new CanonicalPortDefinition
                        {
                            Name = "result",
                            DataType = "string"
                        }
                    ],
                    InputMappings = new Dictionary<string, string>(StringComparer.Ordinal)
                    {
                        ["input"] = "context.value"
                    },
                    InputResolutions = new Dictionary<string, CanonicalInputResolution>(StringComparer.Ordinal)
                    {
                        ["input"] = new()
                        {
                            SourceExpression = "context.value"
                        }
                    },
                    OnFailure = "continue",
                    MaxRetries = 7,
                    RetryDelaySeconds = 12,
                    RetryConfig = new CanonicalRetryConfig
                    {
                        MaxRetries = 7
                    },
                    TimeoutSeconds = 900,
                    RollbackToolId = "tool.rollback",
                    Schedule = new CanonicalScheduleConfig
                    {
                        Kind = "delay",
                        DelaySeconds = 10
                    },
                    HumanReview = new CanonicalHumanReviewConfig
                    {
                        ApproverRoles = ["ops"]
                    },
                    PersistOutput = true
                }
            ],
            Edges =
            [
                new CanonicalWorkflowEdge
                {
                    FromStepId = "step-a",
                    FromPort = "result",
                    ToStepId = "step-b",
                    ToPort = "input",
                    Transform = new CanonicalConnectionTransform
                    {
                        Type = "template",
                        Template = "{{ result }}"
                    }
                }
            ]
        };

        var exception = Assert.Throws<InvalidOperationException>(() => _sut.ApplyCanonicalWorkflowGraph(template, graph));

        Assert.Contains("portable workflow subset", exception.Message, StringComparison.Ordinal);
        Assert.Contains("trigger.inputSchema", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].pieceId", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].pieceType", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].addonVersion", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].inputs", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].outputs", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].inputMappings", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].inputResolutions", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].onFailure", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].maxRetries", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].retryDelaySeconds", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].retryConfig", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].timeoutSeconds", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].rollbackToolId", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].schedule", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].humanReview", exception.Message, StringComparison.Ordinal);
        Assert.Contains("nodes[step-a].persistOutput", exception.Message, StringComparison.Ordinal);
        Assert.Contains("edges[step-a:result->step-b:input].transform", exception.Message, StringComparison.Ordinal);
    }

    private static CanonicalWorkflowGraph BuildCanonicalGraph()
    {
        return new CanonicalWorkflowGraph
        {
            CanonicalFormat = CanonicalWorkflowGraph.DefaultCanonicalFormat,
            CanonicalVersion = 1,
            Trigger = new CanonicalWorkflowTrigger
            {
                Type = "event",
                EventType = "contract.updated",
                InputSchema =
                [
                    new CanonicalPortDefinition
                    {
                        Name = "result",
                        DataType = "object"
                    },
                    new CanonicalPortDefinition
                    {
                        Name = "approval",
                        DataType = "string"
                    }
                ]
            },
            EntryStepId = "step-b",
            Nodes =
            [
                new CanonicalWorkflowNode
                {
                    Id = "step-b",
                    Name = "Approve",
                    Type = "approval",
                    Inputs =
                    [
                        new CanonicalPortDefinition
                        {
                            Name = "zeta",
                            DataType = "string"
                        },
                        new CanonicalPortDefinition
                        {
                            Name = "alpha",
                            DataType = "string"
                        }
                    ],
                    Outputs =
                    [
                        new CanonicalPortDefinition
                        {
                            Name = "zeta",
                            DataType = "string"
                        },
                        new CanonicalPortDefinition
                        {
                            Name = "alpha",
                            DataType = "string"
                        }
                    ],
                    Config = new Dictionary<string, JsonElement>(StringComparer.Ordinal)
                    {
                        ["zeta"] = JsonSerializer.SerializeToElement(true),
                        ["alpha"] = JsonSerializer.SerializeToElement("strict")
                    },
                    InputMappings = new Dictionary<string, string>(StringComparer.Ordinal)
                    {
                        ["zeta"] = "steps.source.outputs.zeta",
                        ["alpha"] = "steps.source.outputs.alpha"
                    },
                    InputResolutions = new Dictionary<string, CanonicalInputResolution>(StringComparer.Ordinal)
                    {
                        ["zeta"] = new()
                        {
                            SourceExpression = "context.zeta",
                            Transform = new CanonicalConnectionTransform
                            {
                                Type = "lookup",
                                LookupTable = new Dictionary<string, string>(StringComparer.Ordinal)
                                {
                                    ["zeta"] = "late",
                                    ["alpha"] = "early"
                                }
                            }
                        },
                        ["alpha"] = new()
                        {
                            SourceExpression = "context.alpha",
                            StaticValue = JsonSerializer.SerializeToElement("fallback")
                        }
                    },
                    HumanReview = new CanonicalHumanReviewConfig
                    {
                        ApproverUserIds = ["zoe", "anna"],
                        ApproverRoles = ["reviewers", "ops"]
                    },
                    RetryConfig = new CanonicalRetryConfig
                    {
                        RetryableErrorCodes = ["transient", "fatal"]
                    }
                },
                new CanonicalWorkflowNode
                {
                    Id = "step-a",
                    Name = "Collect",
                    Type = "task"
                }
            ],
            Edges =
            [
                new CanonicalWorkflowEdge
                {
                    FromStepId = "step-b",
                    FromPort = "approved",
                    ToStepId = "step-c",
                    ToPort = "input",
                    Priority = 20
                },
                new CanonicalWorkflowEdge
                {
                    FromStepId = "step-a",
                    FromPort = "result",
                    ToStepId = "step-b",
                    ToPort = "input",
                    Priority = 10
                }
            ],
            Variables = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal)
            {
                ["zeta"] = new()
                {
                    Name = "zeta",
                    DataType = "string"
                },
                ["alpha"] = new()
                {
                    Name = "alpha",
                    DataType = "string",
                    DefaultValue = JsonSerializer.SerializeToElement("A-123")
                }
            }
        };
    }
}
