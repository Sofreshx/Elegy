using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.Skills.Tests;

public sealed class SkillDefinitionTests
{
    [Fact]
    public void Default_Id_Is_Empty()
    {
        var def = new SkillDefinition();
        Assert.Equal(string.Empty, def.Id);
    }

    [Fact]
    public void Default_LifecycleState_Is_Draft()
    {
        var def = new SkillDefinition();
        Assert.Equal(SkillLifecycleState.Draft, def.LifecycleState);
    }

    [Fact]
    public void Default_Triggers_Is_Empty()
    {
        var def = new SkillDefinition();
        Assert.Empty(def.Triggers);
    }

    [Fact]
    public void Default_Constraints_Is_Empty()
    {
        var def = new SkillDefinition();
        Assert.Empty(def.Constraints);
    }
}
