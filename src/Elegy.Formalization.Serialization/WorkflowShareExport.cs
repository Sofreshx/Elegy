using System.Text.Json;
using Elegy.Formalization.Core.Workflow.Models;

namespace Elegy.Formalization.Serialization;

public sealed record WorkflowShareExport
{
    public const string DefaultCanonicalFormat = "workflow-share-export";
    public const string MermaidProjectionFormat = "mermaid";

    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }

    public WorkflowRepresentationMetadata Representation { get; init; } = new();

    public BlueprintMetadata Blueprint { get; init; } = new();

    public IReadOnlyList<WorkflowTriggerExport> Triggers { get; init; } = [];

    public string? EntryStepId { get; init; }

    public IReadOnlyList<WorkflowStep> Steps { get; init; } = [];

    public IReadOnlyList<WorkflowConnection> Connections { get; init; } = [];

    public IReadOnlyDictionary<string, WorkflowVariableExport> Variables { get; init; } = new Dictionary<string, WorkflowVariableExport>(StringComparer.Ordinal);

    public WorkflowLayout Layout { get; init; } = new();

    public bool StrictValidation { get; init; }

    public static WorkflowShareExport FromDefinition(WorkflowDefinition workflow)
    {
        ArgumentNullException.ThrowIfNull(workflow);

        var blueprint = NormalizeBlueprint(workflow.Blueprint);

        return new WorkflowShareExport
        {
            Name = workflow.Name,
            Description = workflow.Description,
            Representation = new WorkflowRepresentationMetadata
            {
                CanonicalFormat = DefaultCanonicalFormat,
                CanonicalVersion = 1,
                ProjectionFormats = [MermaidProjectionFormat]
            },
            Blueprint = blueprint,
            Triggers = workflow.Triggers
                .Select(WorkflowTriggerExport.FromTrigger)
                .ToArray(),
            EntryStepId = workflow.EntryStepId,
            Steps = workflow.Steps
                .Select(CloneStep)
                .ToArray(),
            Connections = workflow.Connections
                .Select(static connection => connection with { })
                .ToArray(),
            Variables = workflow.Variables
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => WorkflowVariableExport.FromVariable(kvp.Value),
                    StringComparer.Ordinal),
            Layout = workflow.Layout,
            StrictValidation = workflow.StrictValidation
        };
    }

    public WorkflowDefinition ToDefinition(string workflowId)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(workflowId);

        var blueprint = NormalizeBlueprint(Blueprint);
        return new WorkflowDefinition
        {
            Id = workflowId,
            Name = Name,
            Description = Description,
            SpecVersion = blueprint.SpecVersion,
            CanonicalAuthority = blueprint.CanonicalAuthority,
            ConflictPolicy = blueprint.ConflictPolicy,
            Blueprint = blueprint,
            Triggers = Triggers
                .Select(static trigger => trigger.ToTrigger())
                .ToArray(),
            EntryStepId = EntryStepId,
            Steps = Steps
                .Select(CloneStep)
                .ToArray(),
            Connections = Connections
                .Select(static connection => connection with { })
                .ToArray(),
            Variables = Variables
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => kvp.Value.ToVariable(),
                    StringComparer.Ordinal),
            Layout = Layout,
            StrictValidation = StrictValidation
        };
    }

    private static WorkflowStep CloneStep(WorkflowStep step)
    {
        return step with
        {
            Config = step.Config
                .OrderBy(static kvp => kvp.Key, StringComparer.Ordinal)
                .ToDictionary(
                    static kvp => kvp.Key,
                    static kvp => CloneJsonElement(kvp.Value),
                    StringComparer.Ordinal)
        };
    }

    private static BlueprintMetadata NormalizeBlueprint(BlueprintMetadata? blueprint)
    {
        var source = blueprint ?? new BlueprintMetadata();
        var specVersion = string.IsNullOrWhiteSpace(source.SpecVersion)
            ? BlueprintMetadata.DefaultSpecVersion
            : source.SpecVersion.Trim();

        return source with { SpecVersion = specVersion };
    }

    internal static JsonElement CloneJsonElement(JsonElement element)
    {
        return JsonDocument.Parse(element.GetRawText()).RootElement.Clone();
    }
}

public sealed record WorkflowRepresentationMetadata
{
    public string CanonicalFormat { get; init; } = WorkflowShareExport.DefaultCanonicalFormat;

    public int CanonicalVersion { get; init; } = 1;

    public IReadOnlyList<string> ProjectionFormats { get; init; } = [WorkflowShareExport.MermaidProjectionFormat];
}

public sealed record WorkflowTriggerExport
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string Type { get; init; } = string.Empty;

    public string? TargetStepId { get; init; }

    public string? CronExpression { get; init; }

    public string? Timezone { get; init; } = "UTC";

    public string? EventType { get; init; }

    public static WorkflowTriggerExport FromTrigger(WorkflowTrigger trigger)
    {
        ArgumentNullException.ThrowIfNull(trigger);

        return new WorkflowTriggerExport
        {
            Id = trigger.Id,
            Name = trigger.Name,
            Type = trigger.Type,
            TargetStepId = trigger.TargetStepId,
            CronExpression = trigger.CronExpression,
            Timezone = trigger.Timezone,
            EventType = trigger.EventType
        };
    }

    public WorkflowTrigger ToTrigger()
    {
        return new WorkflowTrigger
        {
            Id = Id,
            Name = Name,
            Type = Type,
            TargetStepId = TargetStepId,
            CronExpression = CronExpression,
            Timezone = Timezone,
            EventType = EventType
        };
    }
}

public sealed record WorkflowVariableExport
{
    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }

    public string DataType { get; init; } = "any";

    public JsonElement? DefaultValue { get; init; }

    public bool IsSecret { get; init; }

    public static WorkflowVariableExport FromVariable(WorkflowVariable variable)
    {
        ArgumentNullException.ThrowIfNull(variable);

        return new WorkflowVariableExport
        {
            Name = variable.Name,
            Description = variable.Description,
            DataType = variable.DataType,
            DefaultValue = variable.IsSecret || variable.DefaultValue is null
                ? null
                : WorkflowShareExport.CloneJsonElement(variable.DefaultValue.Value),
            IsSecret = variable.IsSecret
        };
    }

    public WorkflowVariable ToVariable()
    {
        return new WorkflowVariable
        {
            Name = Name,
            Description = Description,
            DataType = DataType,
            DefaultValue = DefaultValue is null
                ? null
                : WorkflowShareExport.CloneJsonElement(DefaultValue.Value),
            IsSecret = false
        };
    }
}
