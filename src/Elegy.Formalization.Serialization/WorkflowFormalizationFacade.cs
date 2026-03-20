using System.Text.Json;
using System.Text.Json.Serialization;
using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Serialization;

public sealed class WorkflowFormalizationFacade : IWorkflowFormalizationFacade
{
    private static readonly JsonSerializerOptions CanonicalGraphSerializeOptions = new()
    {
        PropertyNamingPolicy = JsonNamingPolicy.CamelCase,
        DefaultIgnoreCondition = JsonIgnoreCondition.WhenWritingNull,
        WriteIndented = false
    };

    private static readonly JsonSerializerOptions CanonicalGraphDeserializeOptions = new()
    {
        PropertyNameCaseInsensitive = true,
        AllowTrailingCommas = true,
        ReadCommentHandling = JsonCommentHandling.Skip
    };

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

    public CanonicalWorkflowGraph NormalizeCanonicalWorkflowGraph(WorkflowDefinition workflow)
    {
        return CanonicalWorkflowGraphNormalizer.Normalize(workflow);
    }

    public CanonicalWorkflowGraph NormalizeCanonicalWorkflowGraph(CanonicalWorkflowGraph graph)
    {
        return CanonicalWorkflowGraphNormalizer.Normalize(graph);
    }

    public string SerializeCanonicalWorkflowGraph(CanonicalWorkflowGraph graph)
    {
        ArgumentNullException.ThrowIfNull(graph);
        return JsonSerializer.Serialize(
            CanonicalWorkflowGraphNormalizer.Normalize(graph),
            CanonicalGraphSerializeOptions);
    }

    public CanonicalWorkflowGraph DeserializeCanonicalWorkflowGraph(string json)
    {
        if (string.IsNullOrWhiteSpace(json))
        {
            throw new FormatException("Canonical workflow graph JSON cannot be empty.");
        }

        var graph = JsonSerializer.Deserialize<CanonicalWorkflowGraph>(json, CanonicalGraphDeserializeOptions)
            ?? throw new FormatException("Invalid canonical workflow graph JSON.");

        return CanonicalWorkflowGraphNormalizer.Normalize(graph);
    }

    /// <inheritdoc />
    public WorkflowDefinition ApplyCanonicalWorkflowGraph(WorkflowDefinition workflow, CanonicalWorkflowGraph graph)
    {
        return CanonicalWorkflowGraphNormalizer.Apply(workflow, graph);
    }
}
