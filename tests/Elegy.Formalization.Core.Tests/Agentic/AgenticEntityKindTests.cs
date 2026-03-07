using Elegy.Formalization.Core.Agentic;
using Xunit;

namespace Elegy.Formalization.Core.Tests.Agentic;

public sealed class AgenticEntityKindTests
{
    [Fact]
    public void Default_Value_Is_Skill()
    {
        var value = default(AgenticEntityKind);
        Assert.Equal(AgenticEntityKind.Skill, value);
        Assert.Equal(0, (int)value);
    }

    [Fact]
    public void Has_Exactly_Three_Members()
    {
        var members = Enum.GetValues<AgenticEntityKind>();
        Assert.Equal(3, members.Length);
    }

    [Fact]
    public void Members_Have_Expected_Integer_Values()
    {
        Assert.Equal(0, (int)AgenticEntityKind.Skill);
        Assert.Equal(1, (int)AgenticEntityKind.Agent);
        Assert.Equal(2, (int)AgenticEntityKind.DynamicSkill);
    }
}
