using System.Text;
using System.Text.RegularExpressions;
using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Projections.Mermaid;

public static partial class WorkflowMermaidCodec
{
    public static string Serialize(WorkflowDefinition workflow)
    {
        var lines = new List<string> { "flowchart TD" };
        var stepNodeIds = new Dictionary<string, string>(StringComparer.OrdinalIgnoreCase);

        foreach (var step in workflow.Steps.OrderBy(x => x.Id, StringComparer.Ordinal))
        {
            var nodeId = BuildNodeId("step", step.Id);
            stepNodeIds[step.Id] = nodeId;
            lines.Add($"    {nodeId}[\"{Escape(step.Name)}\"]");
        }

        foreach (var trigger in workflow.Triggers.OrderBy(x => x.Id, StringComparer.Ordinal))
        {
            var triggerNodeId = BuildNodeId("trigger", trigger.Id);
            lines.Add($"    {triggerNodeId}((\"{Escape(trigger.Name)}\"))");

            if (!string.IsNullOrWhiteSpace(trigger.TargetStepId)
                && stepNodeIds.TryGetValue(trigger.TargetStepId, out var targetNodeId))
            {
                lines.Add($"    {triggerNodeId} --> {targetNodeId}");
            }
        }

        foreach (var connection in workflow.Connections
                     .OrderBy(x => x.Id, StringComparer.Ordinal)
                     .ThenBy(x => x.FromStepId, StringComparer.Ordinal)
                     .ThenBy(x => x.ToStepId, StringComparer.Ordinal))
        {
            var fromId = BuildNodeId("step", connection.FromStepId);
            var toId = BuildNodeId("step", connection.ToStepId);
            var edgeLabel = string.IsNullOrWhiteSpace(connection.Label)
                ? string.Empty
                : $"|{Escape(connection.Label)}|";
            lines.Add($"    {fromId} -->{edgeLabel} {toId}");
        }

        var builder = new StringBuilder();
        for (var i = 0; i < lines.Count; i++)
        {
            builder.Append(lines[i]);
            if (i < lines.Count - 1)
            {
                builder.Append('\n');
            }
        }

        return builder.ToString();
    }

    private static string BuildNodeId(string prefix, string rawId)
    {
        var normalized = InvalidNodeIdCharsRegex().Replace(rawId, "_");
        return $"{prefix}_{normalized}";
    }

    private static string Escape(string value)
    {
        return value.Replace("\"", "\\\"", StringComparison.Ordinal);
    }

    [GeneratedRegex("[^A-Za-z0-9_]", RegexOptions.Compiled)]
    private static partial Regex InvalidNodeIdCharsRegex();
}
