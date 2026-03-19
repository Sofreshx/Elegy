using Elegy.Formalization.Core.Workflow.Models;
using Elegy.Formalization.Projections.Mermaid;
using Xunit;

namespace Elegy.Formalization.Projections.Mermaid.Tests;

public sealed class WorkflowMermaidCodecTests
{
    [Fact]
    public void Serialize_Is_Deterministic_For_Unordered_Input()
    {
        var workflow = new WorkflowDefinition
        {
            Steps =
            [
                new WorkflowStep { Id = "step-b", Name = "Second", Type = "task" },
                new WorkflowStep { Id = "step-a", Name = "First", Type = "task" }
            ],
            Connections =
            [
                new WorkflowConnection { Id = "c2", FromStepId = "step-b", FromPort = "result", ToStepId = "step-a", ToPort = "input", Label = "back" },
                new WorkflowConnection { Id = "c1", FromStepId = "step-a", FromPort = "output", ToStepId = "step-b", ToPort = "input", Label = "next", Condition = "approved" }
            ]
        };

        var output = WorkflowMermaidCodec.Serialize(workflow);

        var expected = string.Join('\n',
            "flowchart TD",
            "    step_a[\"First\"]",
            "    step_b[\"Second\"]",
            "    step_a -->|output->input; label:next; when:approved| step_b",
            "    step_b -->|result->input; label:back| step_a");

        Assert.Equal(expected, output);
    }

    [Fact]
    public void Serialize_Normalizes_Node_Ids_And_Ignores_Trigger_Nodes()
    {
        var workflow = new WorkflowDefinition
        {
            Triggers =
            [
                new WorkflowTrigger { Id = "trigger-1", Name = "Manual", Type = "manual", TargetStepId = "1 bad-id" }
            ],
            Steps =
            [
                new WorkflowStep { Id = "1 bad-id", Name = "Start", Type = "task" },
                new WorkflowStep { Id = "1_bad_id", Name = "Shadow", Type = "task" }
            ],
            Connections =
            [
                new WorkflowConnection { FromStepId = "1 bad-id", ToStepId = "1_bad_id" }
            ]
        };

        var output = WorkflowMermaidCodec.Serialize(workflow);

        Assert.DoesNotContain("trigger_1", output);
        Assert.Contains("    n_1_bad_id[\"Start\"]", output);
        Assert.Contains("    n_1_bad_id_2[\"Shadow\"]", output);
        Assert.Contains("    n_1_bad_id --> n_1_bad_id_2", output);
    }
}
