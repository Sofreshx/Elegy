using Elegy.Formalization.Validation;
using Xunit;

namespace Elegy.Formalization.Validation.Tests;

public sealed class WorkflowValidationModeResolverTests
{
    [Fact]
    public void Resolve_Uses_Precedence_Query_Then_Body_Then_Header_Then_Default()
    {
        var result = WorkflowValidationModeResolver.Resolve(
            new Dictionary<string, string?> { [WorkflowValidationModeResolver.ValidationModeQueryParameter] = "strict" },
            bodyMode: "warn",
            headers: new Dictionary<string, string?> { [WorkflowValidationModeResolver.ValidationModeHeader] = "warn" },
            defaultMode: WorkflowValidationMode.Warn);

        Assert.True(result.IsValid);
        Assert.Equal(WorkflowValidationMode.Strict, result.Mode);
        Assert.Equal("strict", result.ModeApplied);
        Assert.True(result.Blocking);
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
        Assert.Equal("warn", result.ModeApplied);
        Assert.False(result.Blocking);
        Assert.Equal("default", result.Source);
    }

    [Fact]
    public void Resolve_Returns_Deterministic_Invalid_Result_When_Selected_Mode_Is_Invalid()
    {
        var result = WorkflowValidationModeResolver.Resolve(
            new Dictionary<string, string?> { [WorkflowValidationModeResolver.ValidationModeQueryParameter] = "lax" },
            bodyMode: "strict",
            headers: null);

        Assert.False(result.IsValid);
        Assert.Equal(WorkflowValidationMode.Warn, result.Mode);
        Assert.Equal("warn", result.ModeApplied);
        Assert.NotNull(result.Invalid);
        Assert.Equal("INVALID_VALIDATION_MODE", result.Invalid!.Code);
        Assert.Equal("Invalid validation mode. Allowed values: warn, strict.", result.Invalid.Message);
        Assert.Equal("lax", result.Invalid.ProvidedMode);
        Assert.Equal("query", result.Invalid.Source);
        Assert.Equal(["warn", "strict"], result.Invalid.AllowedModes);
    }

    [Theory]
    [InlineData(null, "strict", "warn", "body", "strict")]
    [InlineData(null, null, "strict", "header", "strict")]
    [InlineData(null, null, null, "default", "warn")]
    public void Resolve_Falls_Back_Through_Expected_Sources(
        string? queryMode,
        string? bodyMode,
        string? headerMode,
        string expectedSource,
        string expectedMode)
    {
        var query = queryMode is null
            ? null
            : new Dictionary<string, string?> { [WorkflowValidationModeResolver.ValidationModeQueryParameter] = queryMode };
        var headers = headerMode is null
            ? null
            : new Dictionary<string, string?> { [WorkflowValidationModeResolver.ValidationModeHeader] = headerMode };

        var result = WorkflowValidationModeResolver.Resolve(query, bodyMode, headers, WorkflowValidationMode.Warn);

        Assert.True(result.IsValid);
        Assert.Equal(expectedSource, result.Source);
        Assert.Equal(expectedMode, result.ModeApplied);
    }
}
