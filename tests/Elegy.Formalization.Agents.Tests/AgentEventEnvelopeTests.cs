using Elegy.Formalization.Agents;
using Xunit;

namespace Elegy.Formalization.Agents.Tests;

public sealed class AgentEventEnvelopeTests
{
    [Fact]
    public void Default_Source_Is_Broker()
    {
        var envelope = new AgentEventEnvelope();

        Assert.Equal(AgentEventSource.Broker, envelope.Source);
        Assert.NotNull(envelope.Payload);
    }

    [Fact]
    public void Payload_Can_Carry_Delta_And_Usage_Together()
    {
        var payload = new AgentEventPayload
        {
            DeltaContent = "Hello",
            Usage = new AgentUsage { InputTokens = 1, OutputTokens = 2, TotalTokens = 3 }
        };

        Assert.Equal("Hello", payload.DeltaContent);
        Assert.Equal(3, payload.Usage!.TotalTokens);
    }
}