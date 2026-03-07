namespace Elegy.Formalization.AgentFactory;

public sealed record AgentFactoryOptions
{
    public string NamingPattern { get; init; } = @"^[a-z0-9]+(-[a-z0-9]+)*$";
}
