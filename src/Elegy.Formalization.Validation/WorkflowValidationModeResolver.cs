namespace Elegy.Formalization.Validation;

public static class WorkflowValidationModeResolver
{
    private static readonly IReadOnlyList<string> AllowedModes = ["strict", "warn"];

    public static WorkflowValidationModeResolution Resolve(
        IReadOnlyDictionary<string, string?>? queryParameters,
        string? bodyMode,
        IReadOnlyDictionary<string, string?>? headers,
        WorkflowValidationMode defaultMode = WorkflowValidationMode.Strict)
    {
        var candidate = FirstPresent(
            ("query", GetValue(queryParameters, "mode")),
            ("body", bodyMode),
            ("header", GetValue(headers, "x-validation-mode")));

        if (candidate.Value is null)
        {
            return new WorkflowValidationModeResolution
            {
                Mode = defaultMode,
                Source = "default"
            };
        }

        if (TryParseMode(candidate.Value, out var mode))
        {
            return new WorkflowValidationModeResolution
            {
                Mode = mode,
                Source = candidate.Source
            };
        }

        return new WorkflowValidationModeResolution
        {
            Source = candidate.Source,
            Invalid = new InvalidValidationModeResult(
                "invalid_validation_mode",
                $"Validation mode '{candidate.Value}' from {candidate.Source} is invalid.",
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
