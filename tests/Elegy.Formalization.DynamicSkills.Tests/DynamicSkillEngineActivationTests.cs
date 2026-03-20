using Elegy.Formalization.DynamicSkills;
using Xunit;

namespace Elegy.Formalization.DynamicSkills.Tests;

#pragma warning disable CS0618

public sealed class DynamicSkillEngineActivationTests
{
    private readonly DynamicSkillEngine _disabledEngine = new(new DynamicSkillEngineOptions { IsEnabled = false });

    [Fact]
    public void Create_Throws_When_Not_Enabled()
    {
        var ex = Assert.Throws<InvalidOperationException>(() =>
            _disabledEngine.Create(new DynamicSkillCreateRequest { Name = "test" }));
        Assert.Contains("not enabled", ex.Message);
    }
}

#pragma warning restore CS0618
