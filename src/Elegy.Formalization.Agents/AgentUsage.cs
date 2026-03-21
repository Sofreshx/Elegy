namespace Elegy.Formalization.Agents;

public sealed record AgentUsage
{
    public int? InputTokens { get; init; }
    public int? OutputTokens { get; init; }
    public int? TotalTokens { get; init; }
}