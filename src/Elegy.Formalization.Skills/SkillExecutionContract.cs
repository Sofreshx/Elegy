namespace Elegy.Formalization.Skills;

public sealed record SkillExecutionContract
{
    public SkillExecutionMode Mode { get; init; } = SkillExecutionMode.RequestResponse;
    public bool IsDeterministic { get; init; }
    public bool HasSideEffects { get; init; }
    public int? TimeoutSeconds { get; init; }
}