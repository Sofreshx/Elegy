namespace Elegy.Formalization.Validation;

public sealed record InvalidValidationModeResult(
    string Code,
    string Message,
    string ProvidedMode,
    string Source,
    IReadOnlyList<string> AllowedModes);
