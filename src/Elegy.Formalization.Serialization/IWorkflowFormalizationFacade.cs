using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Serialization;

public interface IWorkflowFormalizationFacade
{
    WorkflowShareExport NormalizePortableWorkflow(WorkflowDefinition workflow);

    string SerializePortableWorkflow(WorkflowShareExport workflow);

    WorkflowShareExport DeserializePortableWorkflow(string json);

    WorkflowDefinition DeserializePortableWorkflowAsDefinition(string json, string workflowId);

    CanonicalWorkflowGraph NormalizeCanonicalWorkflowGraph(WorkflowDefinition workflow);

    CanonicalWorkflowGraph NormalizeCanonicalWorkflowGraph(CanonicalWorkflowGraph graph);

    string SerializeCanonicalWorkflowGraph(CanonicalWorkflowGraph graph);

    CanonicalWorkflowGraph DeserializeCanonicalWorkflowGraph(string json);

    /// <summary>
    /// Applies a canonical workflow graph back onto a <see cref="WorkflowDefinition"/> template.
    /// Only the portable workflow subset is supported. Canonical-only members that cannot be
    /// represented by <see cref="WorkflowDefinition"/> cause an <see cref="InvalidOperationException"/>.
    /// </summary>
    WorkflowDefinition ApplyCanonicalWorkflowGraph(WorkflowDefinition workflow, CanonicalWorkflowGraph graph);
}
