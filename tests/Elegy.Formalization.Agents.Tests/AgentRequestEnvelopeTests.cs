using Elegy.Formalization.Agents;
using Xunit;

namespace Elegy.Formalization.Agents.Tests;

public sealed class AgentRequestEnvelopeTests
{
    [Fact]
    public void Default_Request_Enables_Streaming()
    {
        var request = new AgentRequestEnvelope();

        Assert.True(request.StreamingRequested);
        Assert.NotNull(request.Context);
        Assert.Empty(request.Messages);
    }

    [Fact]
    public void Request_Context_Defaults_To_Empty_Collections()
    {
        var context = new AgentRequestContext();

        Assert.Empty(context.CapabilityHints);
        Assert.Empty(context.Metadata);
    }
}