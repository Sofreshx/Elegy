using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Serialization;

public sealed class WorkflowFormalizationFacade : IWorkflowFormalizationFacade
{
    public WorkflowShareExport NormalizePortableWorkflow(WorkflowDefinition workflow)
    {
        return WorkflowShareExport.FromDefinition(workflow);
    }

    public string SerializePortableWorkflow(WorkflowShareExport workflow)
    {
        ArgumentNullException.ThrowIfNull(workflow);
        return CanonicalJsonSerializer.Serialize(workflow);
    }

    public WorkflowShareExport DeserializePortableWorkflow(string json)
    {
        return CanonicalJsonSerializer.Deserialize<WorkflowShareExport>(json);
    }

    public WorkflowDefinition DeserializePortableWorkflowAsDefinition(string json, string workflowId)
    {
        return DeserializePortableWorkflow(json).ToDefinition(workflowId);
    }
}
