using Elegy.Formalization.Validation;
using Xunit;

namespace Elegy.Formalization.Validation.Tests;

public sealed class WorkflowValidationModeResolverTests
{
    [Fact]
    public void Resolve_Uses_Precedence_Query_Then_Body_Then_Header_Then_Default()
    {
        var result = WorkflowValidationModeResolver.Resolve(
            new Dictionary<string, string?> { ["mode"] = "strict" },
            bodyMode: "warn",
            headers: new Dictionary<string, string?> { ["x-validation-mode"] = "warn" },
            defaultMode: WorkflowValidationMode.Warn);

        Assert.True(result.IsValid);
        Assert.Equal(WorkflowValidationMode.Strict, result.Mode);
        Assert.Equal("query", result.Source);
    }

    [Fact]
    public void Resolve_Uses_Default_When_No_Mode_Provided()
    {
        var result = WorkflowValidationModeResolver.Resolve(
            queryParameters: null,
            bodyMode: null,
            headers: null,
            defaultMode: WorkflowValidationMode.Warn);

        Assert.True(result.IsValid);
        Assert.Equal(WorkflowValidationMode.Warn, result.Mode);
        Assert.Equal("default", result.Source);
    }

    [Fact]
    public void Resolve_Returns_Deterministic_Invalid_Result_When_Selected_Mode_Is_Invalid()
    {
        var result = WorkflowValidationModeResolver.Resolve(
            new Dictionary<string, string?> { ["mode"] = "lax" },
            bodyMode: "strict",
            headers: null);

        Assert.False(result.IsValid);
        Assert.Null(result.Mode);
        Assert.NotNull(result.Invalid);
        Assert.Equal("invalid_validation_mode", result.Invalid!.Code);
        Assert.Equal("lax", result.Invalid.ProvidedMode);
        Assert.Equal("query", result.Invalid.Source);
        Assert.Equal(["strict", "warn"], result.Invalid.AllowedModes);
    }
}
