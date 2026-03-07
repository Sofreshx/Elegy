using Elegy.Formalization.DynamicSkills;
using Xunit;

namespace Elegy.Formalization.DynamicSkills.Tests;

public sealed class DynamicSkillEngineCreateTests
{
    private readonly DynamicSkillEngine _engine = new(new DynamicSkillEngineOptions { IsEnabled = true });

    [Fact]
    public void Create_Returns_Success_With_Valid_Request()
    {
        var result = _engine.Create(new DynamicSkillCreateRequest { Name = "my-skill" });
        Assert.True(result.Success);
        Assert.NotNull(result.CreatedSkill);
        Assert.Equal("my-skill", result.CreatedSkill!.Name);
    }

    [Fact]
    public void Create_Returns_Failure_With_Empty_Name()
    {
        var result = _engine.Create(new DynamicSkillCreateRequest { Name = "" });
        Assert.False(result.Success);
        Assert.NotNull(result.ErrorMessage);
    }
}
