using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Serialization;

public interface IWorkflowFormalizationFacade
{
    WorkflowShareExport NormalizePortableWorkflow(WorkflowDefinition workflow);

    string SerializePortableWorkflow(WorkflowShareExport workflow);

    WorkflowShareExport DeserializePortableWorkflow(string json);

    WorkflowDefinition DeserializePortableWorkflowAsDefinition(string json, string workflowId);
}
