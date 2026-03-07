using Elegy.Formalization.Agents;
using Xunit;

namespace Elegy.Formalization.Agents.Tests;

public sealed class RoutingRuleTests
{
    [Fact]
    public void Default_Priority_Is_Zero()
    {
        var rule = new RoutingRule();
        Assert.Equal(0, rule.Priority);
    }
}
