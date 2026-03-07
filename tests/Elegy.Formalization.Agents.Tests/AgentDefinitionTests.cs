using Elegy.Formalization.Agents;
using Xunit;

namespace Elegy.Formalization.Agents.Tests;

public sealed class AgentDefinitionTests
{
    [Fact]
    public void Default_Id_Is_Empty()
    {
        var def = new AgentDefinition();
        Assert.Equal(string.Empty, def.Id);
    }

    [Fact]
    public void Default_Scope_Is_Session()
    {
        var def = new AgentDefinition();
        Assert.Equal(AgentScope.Session, def.Scope);
    }

    [Fact]
    public void Default_Capabilities_Is_Empty()
    {
        var def = new AgentDefinition();
        Assert.Empty(def.Capabilities);
    }

    [Fact]
    public void Default_RoutingRules_Is_Empty()
    {
        var def = new AgentDefinition();
        Assert.Empty(def.RoutingRules);
    }
}
