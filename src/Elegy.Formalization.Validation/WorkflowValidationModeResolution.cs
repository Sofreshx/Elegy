namespace Elegy.Formalization.Validation;

public sealed record WorkflowValidationModeResolution
{
    public WorkflowValidationMode? Mode { get; init; }

    public string? ModeApplied { get; init; }

    public string Source { get; init; } = string.Empty;

    public InvalidValidationModeResult? Invalid { get; init; }

    public bool Blocking => Mode == WorkflowValidationMode.Strict;

    public bool IsValid => Invalid is null;
}
