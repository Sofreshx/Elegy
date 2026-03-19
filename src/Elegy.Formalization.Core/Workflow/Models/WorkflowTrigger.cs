namespace Elegy.Formalization.Core.Workflow.Models;

public sealed record WorkflowTrigger
{
    public string Id { get; init; } = string.Empty;

    public string Name { get; init; } = string.Empty;

    public string Type { get; init; } = string.Empty;

    public string? TargetStepId { get; init; }

    public string? CronExpression { get; init; }

    public string? Timezone { get; init; } = "UTC";

    public string? EventType { get; init; }

    public string? WebhookSecret { get; init; }
}
