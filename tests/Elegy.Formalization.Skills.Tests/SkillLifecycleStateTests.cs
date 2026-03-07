using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.Skills.Tests;

public sealed class SkillLifecycleStateTests
{
    [Fact]
    public void Default_Value_Is_Draft()
    {
        var value = default(SkillLifecycleState);
        Assert.Equal(SkillLifecycleState.Draft, value);
        Assert.Equal(0, (int)value);
    }

    [Fact]
    public void Has_Exactly_Four_Members()
    {
        var members = Enum.GetValues<SkillLifecycleState>();
        Assert.Equal(4, members.Length);
    }
}
