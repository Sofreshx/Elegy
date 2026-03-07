using Elegy.Formalization.Monitoring;
using Xunit;

namespace Elegy.Formalization.Monitoring.Tests;

public sealed class AgenticEventTests
{
    [Fact]
    public void Default_EventId_Is_Empty()
    {
        var evt = new AgenticEvent();
        Assert.Equal(string.Empty, evt.EventId);
    }

    [Fact]
    public void Default_Severity_Is_Trace()
    {
        var evt = new AgenticEvent();
        Assert.Equal(MonitoringSeverity.Trace, evt.Severity);
    }
}
