namespace Elegy.Formalization.Agents;

public sealed record AgentMessage
{
    public string MessageId { get; init; } = string.Empty;
    public AgentMessageRole Role { get; init; } = AgentMessageRole.User;
    public string Content { get; init; } = string.Empty;
    public string? Name { get; init; }
}