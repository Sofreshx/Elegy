using Elegy.Formalization.Agents;
using Xunit;

namespace Elegy.Formalization.Agents.Tests;

public sealed class AgentScopeTests
{
    [Fact]
    public void Default_Value_Is_Session()
    {
        var value = default(AgentScope);
        Assert.Equal(AgentScope.Session, value);
        Assert.Equal(0, (int)value);
    }

    [Fact]
    public void Has_Exactly_Three_Members()
    {
        var members = Enum.GetValues<AgentScope>();
        Assert.Equal(3, members.Length);
    }
}
