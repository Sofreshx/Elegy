namespace Elegy.Formalization.Validation;

public static class WorkflowValidationModeResolver
{
    public const string ValidationModeQueryParameter = "validationMode";
    public const string ValidationModeHeader = "X-Workflow-Validation-Mode";

    private static readonly IReadOnlyList<string> AllowedModes = ["warn", "strict"];

    public static WorkflowValidationModeResolution Resolve(
        IReadOnlyDictionary<string, string?>? queryParameters,
        string? bodyMode,
        IReadOnlyDictionary<string, string?>? headers,
        WorkflowValidationMode defaultMode = WorkflowValidationMode.Warn)
    {
        var candidate = FirstPresent(
            ("query", GetValue(queryParameters, ValidationModeQueryParameter)),
            ("body", bodyMode),
            ("header", GetValue(headers, ValidationModeHeader)));

        if (candidate.Value is null)
        {
            return new WorkflowValidationModeResolution
            {
                Mode = defaultMode,
                ModeApplied = ToModeApplied(defaultMode),
                Source = "default"
            };
        }

        if (TryParseMode(candidate.Value, out var mode))
        {
            return new WorkflowValidationModeResolution
            {
                Mode = mode,
                ModeApplied = ToModeApplied(mode),
                Source = candidate.Source
            };
        }

        return new WorkflowValidationModeResolution
        {
            Mode = defaultMode,
            ModeApplied = ToModeApplied(defaultMode),
            Source = candidate.Source,
            Invalid = new InvalidValidationModeResult(
                "INVALID_VALIDATION_MODE",
                "Invalid validation mode. Allowed values: warn, strict.",
                candidate.Value,
                candidate.Source,
                AllowedModes)
        };
    }

    private static (string Source, string? Value) FirstPresent(params (string Source, string? Value)[] values)
    {
        foreach (var entry in values)
        {
            if (!string.IsNullOrWhiteSpace(entry.Value))
            {
                return (entry.Source, entry.Value!.Trim());
            }
        }

        return ("default", null);
    }

    private static bool TryParseMode(string raw, out WorkflowValidationMode mode)
    {
        if (string.Equals(raw, "strict", StringComparison.OrdinalIgnoreCase))
        {
            mode = WorkflowValidationMode.Strict;
            return true;
        }

        if (string.Equals(raw, "warn", StringComparison.OrdinalIgnoreCase))
        {
            mode = WorkflowValidationMode.Warn;
            return true;
        }

        mode = default;
        return false;
    }

    private static string ToModeApplied(WorkflowValidationMode mode)
    {
        return mode.ToString().ToLowerInvariant();
    }

    private static string? GetValue(IReadOnlyDictionary<string, string?>? values, string key)
    {
        if (values is null)
        {
            return null;
        }

        foreach (var pair in values)
        {
            if (string.Equals(pair.Key, key, StringComparison.OrdinalIgnoreCase))
            {
                return pair.Value;
            }
        }

        return null;
    }
}
