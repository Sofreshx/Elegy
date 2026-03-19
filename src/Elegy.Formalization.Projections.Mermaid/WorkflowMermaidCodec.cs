using System.Text;
using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Projections.Mermaid;

public static class WorkflowMermaidCodec
{
    public const string Format = "mermaid";

    public static string Serialize(WorkflowDefinition workflow)
    {
        ArgumentNullException.ThrowIfNull(workflow);

        var orderedSteps = workflow.Steps
            .OrderBy(static step => step.Id, StringComparer.Ordinal)
            .ThenBy(static step => step.Name, StringComparer.Ordinal)
            .ToArray();

        var orderedConnections = workflow.Connections
            .OrderBy(static connection => connection.FromStepId, StringComparer.Ordinal)
            .ThenBy(static connection => connection.FromPort, StringComparer.Ordinal)
            .ThenBy(static connection => connection.ToStepId, StringComparer.Ordinal)
            .ThenBy(static connection => connection.ToPort, StringComparer.Ordinal)
            .ThenBy(static connection => connection.Label, StringComparer.Ordinal)
            .ThenBy(static connection => connection.Condition, StringComparer.Ordinal)
            .ThenBy(static connection => connection.Priority)
            .ToArray();

        var nodeIds = BuildNodeIdMap(orderedSteps, orderedConnections);

        var lines = new List<string> { "flowchart TD" };

        foreach (var step in orderedSteps)
        {
            lines.Add($"    {nodeIds[step.Id]}[\"{EscapeNodeLabel(step.Name)}\"]");
        }

        foreach (var connection in orderedConnections)
        {
            var edgeLabel = BuildEdgeLabel(connection);
            var labelSegment = string.IsNullOrWhiteSpace(edgeLabel)
                ? string.Empty
                : $"|{EscapeEdgeLabel(edgeLabel)}|";
            lines.Add($"    {nodeIds[connection.FromStepId]} -->{labelSegment} {nodeIds[connection.ToStepId]}");
        }

        return string.Join('\n', lines);
    }

    private static Dictionary<string, string> BuildNodeIdMap(
        IReadOnlyCollection<WorkflowStep> orderedSteps,
        IReadOnlyCollection<WorkflowConnection> orderedConnections)
    {
        var orderedRawIds = orderedSteps
            .Select(static step => step.Id)
            .ToList();

        var referencedIds = orderedConnections
            .SelectMany(static connection => new[] { connection.FromStepId, connection.ToStepId })
            .Where(id => !orderedRawIds.Contains(id, StringComparer.Ordinal))
            .Distinct(StringComparer.Ordinal)
            .OrderBy(static id => id, StringComparer.Ordinal);

        orderedRawIds.AddRange(referencedIds);

        var nodeMap = new Dictionary<string, string>(StringComparer.Ordinal);
        var usedNodeIds = new HashSet<string>(StringComparer.Ordinal);

        foreach (var rawId in orderedRawIds)
        {
            var baseNodeId = ToMermaidNodeId(rawId);
            var nodeId = baseNodeId;
            var suffix = 2;

            while (!usedNodeIds.Add(nodeId))
            {
                nodeId = $"{baseNodeId}_{suffix}";
                suffix++;
            }

            nodeMap[rawId] = nodeId;
        }

        return nodeMap;
    }

    private static string ToMermaidNodeId(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return "node";
        }

        var chars = value
            .Select(static character => char.IsLetterOrDigit(character) || character == '_' ? character : '_')
            .ToArray();
        var normalized = new string(chars);

        if (string.IsNullOrWhiteSpace(normalized))
        {
            return "node";
        }

        return char.IsDigit(normalized[0]) ? $"n_{normalized}" : normalized;
    }

    private static string BuildEdgeLabel(WorkflowConnection connection)
    {
        var parts = new List<string>();

        if (!string.IsNullOrWhiteSpace(connection.FromPort) || !string.IsNullOrWhiteSpace(connection.ToPort))
        {
            parts.Add($"{connection.FromPort}->{connection.ToPort}");
        }

        if (!string.IsNullOrWhiteSpace(connection.Label))
        {
            parts.Add($"label:{connection.Label}");
        }

        if (!string.IsNullOrWhiteSpace(connection.Condition))
        {
            parts.Add($"when:{connection.Condition}");
        }

        return string.Join("; ", parts);
    }

    private static string EscapeNodeLabel(string value) =>
        value
            .Replace("\\", "\\\\", StringComparison.Ordinal)
            .Replace("\"", "\\\"", StringComparison.Ordinal)
            .Replace("\r", " ", StringComparison.Ordinal)
            .Replace("\n", " ", StringComparison.Ordinal);

    private static string EscapeEdgeLabel(string value) =>
        value
            .Replace("|", "/", StringComparison.Ordinal)
            .Replace("\r", " ", StringComparison.Ordinal)
            .Replace("\n", " ", StringComparison.Ordinal);
}
