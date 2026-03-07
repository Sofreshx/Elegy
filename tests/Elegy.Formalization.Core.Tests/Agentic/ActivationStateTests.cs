using Elegy.Formalization.Core.Agentic;
using Xunit;

namespace Elegy.Formalization.Core.Tests.Agentic;

public sealed class ActivationStateTests
{
    [Fact]
    public void Default_Value_Is_Inactive()
    {
        var value = default(ActivationState);
        Assert.Equal(ActivationState.Inactive, value);
        Assert.Equal(0, (int)value);
    }

    [Fact]
    public void Has_Exactly_Three_Members()
    {
        var members = Enum.GetValues<ActivationState>();
        Assert.Equal(3, members.Length);
    }

    [Fact]
    public void Members_Have_Expected_Integer_Values()
    {
        Assert.Equal(0, (int)ActivationState.Inactive);
        Assert.Equal(1, (int)ActivationState.Active);
        Assert.Equal(2, (int)ActivationState.Experimental);
    }
}
