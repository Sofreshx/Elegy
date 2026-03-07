using Elegy.Formalization.Monitoring;
using Xunit;

namespace Elegy.Formalization.Monitoring.Tests;

public sealed class MonitoringSeverityTests
{
    [Fact]
    public void Default_Value_Is_Trace()
    {
        var value = default(MonitoringSeverity);
        Assert.Equal(MonitoringSeverity.Trace, value);
        Assert.Equal(0, (int)value);
    }

    [Fact]
    public void Has_Exactly_Five_Members()
    {
        var members = Enum.GetValues<MonitoringSeverity>();
        Assert.Equal(5, members.Length);
    }
}
