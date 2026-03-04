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
                new WorkflowStep { Id = "b", Name = "Second", Type = "task" },
                new WorkflowStep { Id = "a", Name = "First", Type = "task" }
            ],
            Connections =
            [
                new WorkflowConnection { Id = "c2", FromStepId = "b", ToStepId = "a", Label = "back" },
                new WorkflowConnection { Id = "c1", FromStepId = "a", ToStepId = "b", Label = "next" }
            ]
        };

        var output = WorkflowMermaidCodec.Serialize(workflow);

        var expected = string.Join('\n',
            "flowchart TD",
            "    step_a[\"First\"]",
            "    step_b[\"Second\"]",
            "    step_a -->|next| step_b",
            "    step_b -->|back| step_a");

        Assert.Equal(expected, output);
    }
}
