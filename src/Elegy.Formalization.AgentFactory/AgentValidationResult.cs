namespace Elegy.Formalization.AgentFactory;

public sealed record AgentValidationResult
{
    public bool IsValid { get; init; }
    public IReadOnlyList<string> Findings { get; init; } = [];
}
