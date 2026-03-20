using System.Text.Json;

namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record CanonicalWorkflowGraph
{
    public const string DefaultCanonicalFormat = "canonical-workflow-graph";

    public string CanonicalFormat { get; init; } = DefaultCanonicalFormat;

    public int CanonicalVersion { get; init; } = 1;

    public CanonicalWorkflowTrigger? Trigger { get; init; }

    public string? EntryStepId { get; init; }

    public IReadOnlyList<CanonicalWorkflowNode> Nodes { get; init; } = [];

    public IReadOnlyList<CanonicalWorkflowEdge> Edges { get; init; } = [];

    public IReadOnlyDictionary<string, WorkflowVariable> Variables { get; init; } = new Dictionary<string, WorkflowVariable>(StringComparer.Ordinal);
}

public sealed record CanonicalWorkflowTrigger
{
    public string Type { get; init; } = "manual";

    public string? CronExpression { get; init; }

    public string? Timezone { get; init; } = "UTC";

    public string? EventType { get; init; }

    public IReadOnlyList<CanonicalPortDefinition> InputSchema { get; init; } = [];
}

public sealed record CanonicalWorkflowNode
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string? Description { get; init; }

    public string Type { get; init; } = string.Empty;

    public string? PieceId { get; init; }

    public string? PieceType { get; init; }

    public string? ToolId { get; init; }

    public int? AddonVersion { get; init; }

    public IReadOnlyList<CanonicalPortDefinition> Inputs { get; init; } = [];

    public IReadOnlyList<CanonicalPortDefinition> Outputs { get; init; } = [];

    public IReadOnlyDictionary<string, JsonElement> Config { get; init; } = new Dictionary<string, JsonElement>(StringComparer.Ordinal);

    public IReadOnlyDictionary<string, string> InputMappings { get; init; } = new Dictionary<string, string>(StringComparer.Ordinal);

    public IReadOnlyDictionary<string, CanonicalInputResolution> InputResolutions { get; init; } = new Dictionary<string, CanonicalInputResolution>(StringComparer.Ordinal);

    public string OnFailure { get; init; } = "stopWorkflow";

    public int MaxRetries { get; init; } = 3;

    public int RetryDelaySeconds { get; init; } = 5;

    public CanonicalRetryConfig? RetryConfig { get; init; }

    public int TimeoutSeconds { get; init; } = 300;

    public string? Condition { get; init; }

    public string? RollbackToolId { get; init; }

    public CanonicalScheduleConfig? Schedule { get; init; }

    public CanonicalHumanReviewConfig? HumanReview { get; init; }

    public bool PersistOutput { get; init; }

    public bool IsEnabled { get; init; } = true;
}

public sealed record CanonicalWorkflowEdge
{
    public string FromStepId { get; init; } = string.Empty;

    public string FromPort { get; init; } = string.Empty;

    public string ToStepId { get; init; } = string.Empty;

    public string ToPort { get; init; } = string.Empty;

    public CanonicalConnectionTransform? Transform { get; init; }

    public string? Condition { get; init; }

    public string? Label { get; init; }

    public int Priority { get; init; }
}

public sealed record CanonicalPortDefinition
{
    public string Name { get; init; } = string.Empty;

    public string? Label { get; init; }

    public string? Description { get; init; }

    public JsonElement? TypeDescriptor { get; init; }

    public string DataType { get; init; } = "any";

    public bool Required { get; init; }

    public JsonElement? DefaultValue { get; init; }

    public bool AllowMultiple { get; init; }

    public string? Schema { get; init; }
}

public sealed record CanonicalInputResolution
{
    public string? SourceExpression { get; init; }

    public JsonElement? StaticValue { get; init; }

    public CanonicalConnectionTransform? Transform { get; init; }

    public JsonElement? DefaultValue { get; init; }
}

public sealed record CanonicalScheduleConfig
{
    public string? Kind { get; init; }

    public int? DelaySeconds { get; init; }

    public DateTimeOffset? ExecuteAt { get; init; }

    public string? CronExpression { get; init; }

    public int? IntervalValue { get; init; }

    public string? IntervalUnit { get; init; }

    public DateTimeOffset? StartAt { get; init; }

    public DateTimeOffset? EndAt { get; init; }

    public int? MaxOccurrences { get; init; }

    public string Timezone { get; init; } = "UTC";
}

public sealed record CanonicalHumanReviewConfig
{
    public IReadOnlyList<string> ApproverUserIds { get; init; } = [];

    public IReadOnlyList<string> ApproverRoles { get; init; } = [];

    public string? Instructions { get; init; }

    public int? TimeoutHours { get; init; }

    public bool SendNotification { get; init; } = true;
}

public sealed record CanonicalConnectionTransform
{
    public string Type { get; init; } = "direct";

    public string? Template { get; init; }

    public IReadOnlyDictionary<string, string>? LookupTable { get; init; }

    public JsonElement? TargetType { get; init; }
}

public sealed record CanonicalRetryConfig
{
    public int MaxRetries { get; init; } = 3;

    public string InitialDelay { get; init; } = "00:00:01";

    public string MaxDelay { get; init; } = "00:05:00";

    public double BackoffMultiplier { get; init; } = 2.0;

    public IReadOnlyList<string>? RetryableErrorCodes { get; init; }
}
