namespace Elegy.Formalization.Agents;

public sealed record AgentEventPayload
{
    public string? MessageId { get; init; }
    public AgentMessageRole? Role { get; init; }
    public string? ToolCallId { get; init; }
    public string? ToolName { get; init; }
    public string? Content { get; init; }
    public string? DeltaContent { get; init; }
    public string? ErrorCode { get; init; }
    public string? ErrorMessage { get; init; }
    public AgentUsage? Usage { get; init; }
    public IReadOnlyDictionary<string, string>? Metadata { get; init; }
}