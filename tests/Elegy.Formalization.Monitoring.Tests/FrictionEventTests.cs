using Elegy.Formalization.Core.Agentic;
using Elegy.Formalization.Monitoring;
using Xunit;

namespace Elegy.Formalization.Monitoring.Tests;

public sealed class FrictionEventTests
{
    [Fact]
    public void FromFrictionEntry_ReturnsCorrectCategory()
    {
        var evt = FrictionEvent.FromFrictionEntry("Bad pattern", "Brittle coupling", "high", "src/foo.cs");

        Assert.Equal(EventCategory.Friction, evt.Category);
    }

    [Fact]
    public void FromFrictionEntry_SetsEntityKindToSkill()
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "reason", "low", "ctx");

        Assert.Equal(AgenticEntityKind.Skill, evt.EntityKind);
    }

    [Fact]
    public void FromFrictionEntry_SetsEntityIdToImplementationFriction()
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "reason", "low", "ctx");

        Assert.Equal("implementation-friction", evt.EntityId);
    }

    [Theory]
    [InlineData("low", MonitoringSeverity.Info)]
    [InlineData("medium", MonitoringSeverity.Warning)]
    [InlineData("high", MonitoringSeverity.Error)]
    [InlineData("critical", MonitoringSeverity.Critical)]
    public void FromFrictionEntry_MapsSeverityCorrectly(string importance, MonitoringSeverity expected)
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "reason", importance, "ctx");

        Assert.Equal(expected, evt.Severity);
    }

    [Fact]
    public void FromFrictionEntry_UnknownImportance_DefaultsToInfo()
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "reason", "unknown", "ctx");

        Assert.Equal(MonitoringSeverity.Info, evt.Severity);
    }

    [Fact]
    public void FromFrictionEntry_MetadataIncludesReasonAndContext()
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "Coupling issue", "high", "src/bar.cs");

        Assert.NotNull(evt.Metadata);
        Assert.Equal("Coupling issue", evt.Metadata!["reason"]);
        Assert.Equal("src/bar.cs", evt.Metadata["context"]);
    }

    [Fact]
    public void FromFrictionEntry_WithClusterId_IncludesInMetadata()
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "reason", "medium", "ctx", clusterId: "CLU-001");

        Assert.NotNull(evt.Metadata);
        Assert.Equal("CLU-001", evt.Metadata!["clusterId"]);
    }

    [Fact]
    public void FromFrictionEntry_WithoutClusterId_ExcludesFromMetadata()
    {
        var evt = FrictionEvent.FromFrictionEntry("title", "reason", "low", "ctx");

        Assert.NotNull(evt.Metadata);
        Assert.False(evt.Metadata!.ContainsKey("clusterId"));
    }

    [Fact]
    public void FromFrictionEntry_GeneratesUniqueEventId()
    {
        var evt1 = FrictionEvent.FromFrictionEntry("title1", "reason", "low", "ctx");
        var evt2 = FrictionEvent.FromFrictionEntry("title2", "reason", "low", "ctx");

        Assert.NotEqual(evt1.EventId, evt2.EventId);
        Assert.StartsWith("friction-", evt1.EventId);
    }

    [Fact]
    public void FromFrictionEntry_SetsMessageToTitle()
    {
        var evt = FrictionEvent.FromFrictionEntry("Shaky pattern detected", "reason", "low", "ctx");

        Assert.Equal("Shaky pattern detected", evt.Message);
    }
}
