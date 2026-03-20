using System.Text.Json;
using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Serialization;

internal static class CanonicalWorkflowGraphNormalizer
{
    public static CanonicalWorkflowGraph Normalize(WorkflowDefinition workflow)
    {
        ArgumentNullException.ThrowIfNull(workflow);

        if (workflow.Triggers.Count > 1)
        {
            throw new InvalidOperationException("Canonical workflow graph normalization supports workflows with at most one trigger.");
        }

        return Normalize(new CanonicalWorkflowGraph
        {
            Trigger = CloneTrigger(workflow.Triggers.FirstOrDefault()),
            EntryStepId = workflow.EntryStepId,
            Nodes = workflow.Steps
                .Select(CloneNode)
                .ToArray(),
            Edges = workflow.Connections
                .Select(CloneEdge)
                .ToArray(),
            Variables = workflow.Variables
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneVariable(kvp.Value),
                    StringComparer.Ordinal)
        });
    }

    public static CanonicalWorkflowGraph Normalize(CanonicalWorkflowGraph graph)
    {
        ArgumentNullException.ThrowIfNull(graph);

        return new CanonicalWorkflowGraph
        {
            CanonicalFormat = string.IsNullOrWhiteSpace(graph.CanonicalFormat)
                ? CanonicalWorkflowGraph.DefaultCanonicalFormat
                : graph.CanonicalFormat.Trim(),
            CanonicalVersion = graph.CanonicalVersion <= 0 ? 1 : graph.CanonicalVersion,
            Trigger = CloneTrigger(graph.Trigger),
            EntryStepId = graph.EntryStepId,
            Nodes = (graph.Nodes ?? [])
                .Select(CloneNode)
                .OrderBy(static node => node.Id, StringComparer.Ordinal)
                .ThenBy(static node => node.Name, StringComparer.Ordinal)
                .ToArray(),
            Edges = (graph.Edges ?? [])
                .Select(CloneEdge)
                .OrderBy(static edge => edge.FromStepId, StringComparer.Ordinal)
                .ThenBy(static edge => edge.FromPort, StringComparer.Ordinal)
                .ThenBy(static edge => edge.ToStepId, StringComparer.Ordinal)
                .ThenBy(static edge => edge.ToPort, StringComparer.Ordinal)
                .ThenBy(static edge => edge.Priority)
                .ThenBy(static edge => edge.Label, StringComparer.Ordinal)
                .ThenBy(static edge => edge.Condition, StringComparer.Ordinal)
                .ToArray(),
            Variables = (graph.Variables ?? new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal))
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneVariable(kvp.Value),
                    StringComparer.Ordinal)
                .ToSortedDictionary()
        };
    }

    public static WorkflowDefinition Apply(WorkflowDefinition workflow, CanonicalWorkflowGraph graph)
    {
        ArgumentNullException.ThrowIfNull(workflow);
        ArgumentNullException.ThrowIfNull(graph);

        var normalized = Normalize(graph);
        EnsurePortableWorkflowCompatibility(normalized);
        return workflow with
        {
            Triggers = normalized.Trigger is null ? [] : [ToWorkflowTrigger(normalized.Trigger)],
            EntryStepId = normalized.EntryStepId,
            Steps = normalized.Nodes
                .Select(ToWorkflowStep)
                .ToArray(),
            Connections = normalized.Edges
                .Select(ToWorkflowConnection)
                .ToArray(),
            Variables = normalized.Variables
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneVariable(kvp.Value),
                    StringComparer.Ordinal)
        };
    }

    private static void EnsurePortableWorkflowCompatibility(CanonicalWorkflowGraph graph)
    {
        var unsupportedMembers = new List<string>();

        if (graph.Trigger?.InputSchema.Count > 0)
        {
            unsupportedMembers.Add("trigger.inputSchema");
        }

        foreach (var node in graph.Nodes)
        {
            unsupportedMembers.AddRange(GetUnsupportedNodeMembers(node));
        }

        foreach (var edge in graph.Edges)
        {
            if (edge.Transform is not null)
            {
                unsupportedMembers.Add($"edges[{GetEdgeIdentity(edge)}].transform");
            }
        }

        if (unsupportedMembers.Count > 0)
        {
            throw new InvalidOperationException(
                "ApplyCanonicalWorkflowGraph only supports the portable workflow subset. " +
                "The current graph contains canonical-only members that cannot be represented by WorkflowDefinition: " +
                string.Join(", ", unsupportedMembers));
        }
    }

    private static IReadOnlyList<string> GetUnsupportedNodeMembers(CanonicalWorkflowNode node)
    {
        var unsupportedMembers = new List<string>();
        var nodePrefix = $"nodes[{node.Id}]";

        if (!string.IsNullOrWhiteSpace(node.PieceId))
        {
            unsupportedMembers.Add($"{nodePrefix}.pieceId");
        }

        if (!string.IsNullOrWhiteSpace(node.PieceType))
        {
            unsupportedMembers.Add($"{nodePrefix}.pieceType");
        }

        if (node.AddonVersion is not null)
        {
            unsupportedMembers.Add($"{nodePrefix}.addonVersion");
        }

        if (node.Inputs.Count > 0)
        {
            unsupportedMembers.Add($"{nodePrefix}.inputs");
        }

        if (node.Outputs.Count > 0)
        {
            unsupportedMembers.Add($"{nodePrefix}.outputs");
        }

        if (node.InputMappings.Count > 0)
        {
            unsupportedMembers.Add($"{nodePrefix}.inputMappings");
        }

        if (node.InputResolutions.Count > 0)
        {
            unsupportedMembers.Add($"{nodePrefix}.inputResolutions");
        }

        if (!string.Equals(node.OnFailure, "stopWorkflow", StringComparison.Ordinal))
        {
            unsupportedMembers.Add($"{nodePrefix}.onFailure");
        }

        if (node.MaxRetries != 3)
        {
            unsupportedMembers.Add($"{nodePrefix}.maxRetries");
        }

        if (node.RetryDelaySeconds != 5)
        {
            unsupportedMembers.Add($"{nodePrefix}.retryDelaySeconds");
        }

        if (node.RetryConfig is not null)
        {
            unsupportedMembers.Add($"{nodePrefix}.retryConfig");
        }

        if (node.TimeoutSeconds != 300)
        {
            unsupportedMembers.Add($"{nodePrefix}.timeoutSeconds");
        }

        if (!string.IsNullOrWhiteSpace(node.RollbackToolId))
        {
            unsupportedMembers.Add($"{nodePrefix}.rollbackToolId");
        }

        if (node.Schedule is not null)
        {
            unsupportedMembers.Add($"{nodePrefix}.schedule");
        }

        if (node.HumanReview is not null)
        {
            unsupportedMembers.Add($"{nodePrefix}.humanReview");
        }

        if (node.PersistOutput)
        {
            unsupportedMembers.Add($"{nodePrefix}.persistOutput");
        }

        return unsupportedMembers;
    }

    private static string GetEdgeIdentity(CanonicalWorkflowEdge edge)
    {
        return $"{edge.FromStepId}:{edge.FromPort}->{edge.ToStepId}:{edge.ToPort}";
    }

    private static CanonicalWorkflowTrigger CloneTrigger(WorkflowTrigger? trigger)
    {
        if (trigger is null)
        {
            return null;
        }

        return new CanonicalWorkflowTrigger
        {
            Type = string.IsNullOrWhiteSpace(trigger.Type) ? "manual" : trigger.Type,
            CronExpression = trigger.CronExpression,
            Timezone = string.IsNullOrWhiteSpace(trigger.Timezone) ? "UTC" : trigger.Timezone,
            EventType = trigger.EventType
        };
    }

    private static CanonicalWorkflowTrigger CloneTrigger(CanonicalWorkflowTrigger? trigger)
    {
        if (trigger is null)
        {
            return null;
        }

        return new CanonicalWorkflowTrigger
        {
            Type = string.IsNullOrWhiteSpace(trigger.Type) ? "manual" : trigger.Type,
            CronExpression = trigger.CronExpression,
            Timezone = string.IsNullOrWhiteSpace(trigger.Timezone) ? "UTC" : trigger.Timezone,
            EventType = trigger.EventType,
            InputSchema = (trigger.InputSchema ?? [])
                .Select(ClonePort)
                .OrderBy(static port => port.Name, StringComparer.Ordinal)
                .ThenBy(static port => port.Label, StringComparer.Ordinal)
                .ToArray()
        };
    }

    private static WorkflowTrigger ToWorkflowTrigger(CanonicalWorkflowTrigger trigger)
    {
        return new WorkflowTrigger
        {
            Type = string.IsNullOrWhiteSpace(trigger.Type) ? "manual" : trigger.Type,
            CronExpression = trigger.CronExpression,
            Timezone = string.IsNullOrWhiteSpace(trigger.Timezone) ? "UTC" : trigger.Timezone,
            EventType = trigger.EventType
        };
    }

    private static CanonicalWorkflowNode CloneNode(WorkflowStep step)
    {
        return new CanonicalWorkflowNode
        {
            Id = step.Id,
            Name = step.Name,
            Description = step.Description,
            Type = step.Type,
            ToolId = step.ToolId,
            Config = step.Config
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneJsonElement(kvp.Value),
                    StringComparer.Ordinal)
                .ToSortedDictionary(),
            Condition = step.Condition,
            IsEnabled = step.IsEnabled
        };
    }

    private static CanonicalWorkflowNode CloneNode(CanonicalWorkflowNode node)
    {
        return new CanonicalWorkflowNode
        {
            Id = node.Id,
            Name = node.Name,
            Description = node.Description,
            Type = node.Type,
            PieceId = node.PieceId,
            PieceType = node.PieceType,
            ToolId = node.ToolId,
            AddonVersion = node.AddonVersion,
            Inputs = (node.Inputs ?? [])
                .Select(ClonePort)
                .OrderBy(static port => port.Name, StringComparer.Ordinal)
                .ThenBy(static port => port.Label, StringComparer.Ordinal)
                .ToArray(),
            Outputs = (node.Outputs ?? [])
                .Select(ClonePort)
                .OrderBy(static port => port.Name, StringComparer.Ordinal)
                .ThenBy(static port => port.Label, StringComparer.Ordinal)
                .ToArray(),
            Config = (node.Config ?? new Dictionary<string, JsonElement>(StringComparer.Ordinal))
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneJsonElement(kvp.Value),
                    StringComparer.Ordinal)
                .ToSortedDictionary(),
            InputMappings = (node.InputMappings ?? new Dictionary<string, string>(StringComparer.Ordinal))
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => kvp.Value,
                    StringComparer.Ordinal)
                .ToSortedDictionary(),
            InputResolutions = (node.InputResolutions ?? new Dictionary<string, CanonicalInputResolution>(StringComparer.Ordinal))
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneInputResolution(kvp.Value),
                    StringComparer.Ordinal)
                .ToSortedDictionary(),
            OnFailure = string.IsNullOrWhiteSpace(node.OnFailure) ? "stopWorkflow" : node.OnFailure,
            MaxRetries = node.MaxRetries,
            RetryDelaySeconds = node.RetryDelaySeconds,
            RetryConfig = CloneRetryConfig(node.RetryConfig),
            TimeoutSeconds = node.TimeoutSeconds <= 0 ? 300 : node.TimeoutSeconds,
            Condition = node.Condition,
            RollbackToolId = node.RollbackToolId,
            Schedule = CloneSchedule(node.Schedule),
            HumanReview = CloneHumanReview(node.HumanReview),
            PersistOutput = node.PersistOutput,
            IsEnabled = node.IsEnabled
        };
    }

    private static WorkflowStep ToWorkflowStep(CanonicalWorkflowNode node)
    {
        return new WorkflowStep
        {
            Id = node.Id,
            Name = node.Name,
            Description = node.Description,
            Type = node.Type,
            ToolId = node.ToolId,
            Config = (node.Config ?? new Dictionary<string, JsonElement>(StringComparer.Ordinal))
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneJsonElement(kvp.Value),
                    StringComparer.Ordinal),
            Condition = node.Condition,
            IsEnabled = node.IsEnabled
        };
    }

    private static CanonicalWorkflowEdge CloneEdge(WorkflowConnection connection)
    {
        return new CanonicalWorkflowEdge
        {
            FromStepId = connection.FromStepId,
            FromPort = connection.FromPort,
            ToStepId = connection.ToStepId,
            ToPort = connection.ToPort,
            Condition = connection.Condition,
            Label = connection.Label,
            Priority = connection.Priority
        };
    }

    private static CanonicalWorkflowEdge CloneEdge(CanonicalWorkflowEdge edge)
    {
        return new CanonicalWorkflowEdge
        {
            FromStepId = edge.FromStepId,
            FromPort = edge.FromPort,
            ToStepId = edge.ToStepId,
            ToPort = edge.ToPort,
            Transform = CloneTransform(edge.Transform),
            Condition = edge.Condition,
            Label = edge.Label,
            Priority = edge.Priority
        };
    }

    private static WorkflowConnection ToWorkflowConnection(CanonicalWorkflowEdge edge)
    {
        return new WorkflowConnection
        {
            Id = string.Empty,
            FromStepId = edge.FromStepId,
            FromPort = edge.FromPort,
            ToStepId = edge.ToStepId,
            ToPort = edge.ToPort,
            Condition = edge.Condition,
            Label = edge.Label,
            Priority = edge.Priority
        };
    }

    private static CanonicalPortDefinition ClonePort(CanonicalPortDefinition port)
    {
        return new CanonicalPortDefinition
        {
            Name = port.Name,
            Label = port.Label,
            Description = port.Description,
            TypeDescriptor = port.TypeDescriptor is null ? null : CloneJsonElement(port.TypeDescriptor.Value),
            DataType = string.IsNullOrWhiteSpace(port.DataType) ? "any" : port.DataType,
            Required = port.Required,
            DefaultValue = port.DefaultValue is null ? null : CloneJsonElement(port.DefaultValue.Value),
            AllowMultiple = port.AllowMultiple,
            Schema = port.Schema
        };
    }

    private static CanonicalInputResolution CloneInputResolution(CanonicalInputResolution resolution)
    {
        return new CanonicalInputResolution
        {
            SourceExpression = resolution.SourceExpression,
            StaticValue = resolution.StaticValue is null ? null : CloneJsonElement(resolution.StaticValue.Value),
            Transform = CloneTransform(resolution.Transform),
            DefaultValue = resolution.DefaultValue is null ? null : CloneJsonElement(resolution.DefaultValue.Value)
        };
    }

    private static CanonicalScheduleConfig? CloneSchedule(CanonicalScheduleConfig? schedule)
    {
        if (schedule is null)
        {
            return null;
        }

        return new CanonicalScheduleConfig
        {
            Kind = schedule.Kind,
            DelaySeconds = schedule.DelaySeconds,
            ExecuteAt = schedule.ExecuteAt,
            CronExpression = schedule.CronExpression,
            IntervalValue = schedule.IntervalValue,
            IntervalUnit = schedule.IntervalUnit,
            StartAt = schedule.StartAt,
            EndAt = schedule.EndAt,
            MaxOccurrences = schedule.MaxOccurrences,
            Timezone = string.IsNullOrWhiteSpace(schedule.Timezone) ? "UTC" : schedule.Timezone
        };
    }

    private static CanonicalHumanReviewConfig? CloneHumanReview(CanonicalHumanReviewConfig? review)
    {
        if (review is null)
        {
            return null;
        }

        return new CanonicalHumanReviewConfig
        {
            ApproverUserIds = (review.ApproverUserIds ?? [])
                .Where(static value => !string.IsNullOrWhiteSpace(value))
                .OrderBy(static value => value, StringComparer.Ordinal)
                .ToArray(),
            ApproverRoles = (review.ApproverRoles ?? [])
                .Where(static value => !string.IsNullOrWhiteSpace(value))
                .OrderBy(static value => value, StringComparer.Ordinal)
                .ToArray(),
            Instructions = review.Instructions,
            TimeoutHours = review.TimeoutHours,
            SendNotification = review.SendNotification
        };
    }

    private static CanonicalRetryConfig? CloneRetryConfig(CanonicalRetryConfig? retry)
    {
        if (retry is null)
        {
            return null;
        }

        return new CanonicalRetryConfig
        {
            MaxRetries = retry.MaxRetries,
            InitialDelay = string.IsNullOrWhiteSpace(retry.InitialDelay) ? "00:00:01" : retry.InitialDelay,
            MaxDelay = string.IsNullOrWhiteSpace(retry.MaxDelay) ? "00:05:00" : retry.MaxDelay,
            BackoffMultiplier = retry.BackoffMultiplier <= 0 ? 2.0 : retry.BackoffMultiplier,
            RetryableErrorCodes = retry.RetryableErrorCodes?
                .Where(static value => !string.IsNullOrWhiteSpace(value))
                .OrderBy(static value => value, StringComparer.Ordinal)
                .ToArray()
        };
    }

    private static CanonicalConnectionTransform? CloneTransform(CanonicalConnectionTransform? transform)
    {
        if (transform is null)
        {
            return null;
        }

        return new CanonicalConnectionTransform
        {
            Type = string.IsNullOrWhiteSpace(transform.Type) ? "direct" : transform.Type,
            Template = transform.Template,
            LookupTable = transform.LookupTable is null
                ? null
                : transform.LookupTable
                    .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                    .ToDictionary(
                        static kvp => kvp.Key,
                        static kvp => kvp.Value,
                        StringComparer.Ordinal)
                    .ToSortedDictionary(),
            TargetType = transform.TargetType is null ? null : CloneJsonElement(transform.TargetType.Value)
        };
    }

    private static WorkflowVariable CloneVariable(WorkflowVariable variable)
    {
        return new WorkflowVariable
        {
            Name = variable.Name,
            Description = variable.Description,
            DataType = string.IsNullOrWhiteSpace(variable.DataType) ? "any" : variable.DataType,
            DefaultValue = variable.IsSecret || variable.DefaultValue is null
                ? null
                : CloneJsonElement(variable.DefaultValue.Value),
            IsSecret = variable.IsSecret
        };
    }

    private static JsonElement CloneJsonElement(JsonElement element)
    {
        using var stream = new MemoryStream();
        using (var writer = new Utf8JsonWriter(stream))
        {
            WriteCanonicalJson(element, writer);
        }

        return JsonDocument.Parse(stream.ToArray()).RootElement.Clone();
    }

    private static void WriteCanonicalJson(JsonElement element, Utf8JsonWriter writer)
    {
        switch (element.ValueKind)
        {
            case JsonValueKind.Object:
                writer.WriteStartObject();
                foreach (var property in element.EnumerateObject().OrderBy(static property => property.Name, StringComparer.Ordinal))
                {
                    writer.WritePropertyName(property.Name);
                    WriteCanonicalJson(property.Value, writer);
                }
                writer.WriteEndObject();
                break;

            case JsonValueKind.Array:
                writer.WriteStartArray();
                foreach (var item in element.EnumerateArray())
                {
                    WriteCanonicalJson(item, writer);
                }
                writer.WriteEndArray();
                break;

            default:
                element.WriteTo(writer);
                break;
        }
    }
}

internal static class CanonicalWorkflowGraphDictionaryExtensions
{
    public static SortedDictionary<string, TValue> ToSortedDictionary<TValue>(this IDictionary<string, TValue> source)
    {
        return new SortedDictionary<string, TValue>(source, StringComparer.Ordinal);
    }
}
