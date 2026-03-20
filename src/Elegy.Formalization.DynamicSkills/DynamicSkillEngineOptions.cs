namespace Elegy.Formalization.DynamicSkills;

[Obsolete("Compatibility surface only. Prefer canonical skill authority or governed artifacts for durable contracts.")]
public sealed record DynamicSkillEngineOptions
{
    public bool IsEnabled { get; init; }
}
