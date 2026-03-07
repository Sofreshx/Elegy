using System.Text.RegularExpressions;
using Elegy.Formalization.Agents;

namespace Elegy.Formalization.AgentFactory;

public sealed class AgentFactoryService
{
    private readonly AgentFactoryOptions _options;

    public AgentFactoryService(AgentFactoryOptions options)
    {
        _options = options;
    }

    public AgentCreateResult Create(AgentCreateRequest request)
    {
        var findings = CollectFindings(
            request.Name,
            request.Description,
            request.Capabilities,
            request.RoutingRules);

        if (findings.Count > 0)
        {
            return new AgentCreateResult
            {
                Success = false,
                Findings = findings
            };
        }

        var agent = new AgentDefinition
        {
            Id = Guid.NewGuid().ToString(),
            Name = request.Name,
            Description = request.Description,
            Capabilities = request.Capabilities,
            RoutingRules = request.RoutingRules,
            Scope = request.Scope
        };

        return new AgentCreateResult
        {
            Success = true,
            CreatedAgent = agent,
            Findings = findings
        };
    }

    public AgentValidationResult Validate(AgentDefinition definition)
    {
        var findings = CollectFindings(
            definition.Name,
            definition.Description,
            definition.Capabilities,
            definition.RoutingRules);

        return new AgentValidationResult
        {
            IsValid = findings.Count == 0,
            Findings = findings
        };
    }

    private List<string> CollectFindings(
        string name,
        string? description,
        IReadOnlyList<AgentCapability> capabilities,
        IReadOnlyList<RoutingRule> routingRules)
    {
        var findings = new List<string>();

        if (string.IsNullOrWhiteSpace(name))
        {
            findings.Add("Name is required.");
        }
        else if (!Regex.IsMatch(name, _options.NamingPattern))
        {
            findings.Add("Name must match kebab-case pattern.");
        }

        if (string.IsNullOrWhiteSpace(description))
        {
            findings.Add("Description is required.");
        }

        if (capabilities.Count == 0)
        {
            findings.Add("At least one capability is required.");
        }
        else
        {
            var duplicateCapabilityIds = capabilities
                .GroupBy(c => c.CapabilityId)
                .Where(g => g.Count() > 1)
                .Select(g => g.Key);

            foreach (var id in duplicateCapabilityIds)
            {
                findings.Add($"Duplicate capability ID: {id}");
            }
        }

        var duplicateRuleIds = routingRules
            .GroupBy(r => r.RuleId)
            .Where(g => g.Count() > 1)
            .Select(g => g.Key);

        foreach (var id in duplicateRuleIds)
        {
            findings.Add($"Duplicate routing rule ID: {id}");
        }

        return findings;
    }
}
