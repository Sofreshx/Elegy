using System.Text.RegularExpressions;
using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

public sealed class SkillForgeService
{
    private readonly DynamicSkillEngine _engine;
    private readonly SkillForgeOptions _options;

    public SkillForgeService(DynamicSkillEngine engine, SkillForgeOptions options)
    {
        _engine = engine;
        _options = options;
    }

    public SkillForgeResult Forge(SkillForgeRequest request)
    {
        if (!Regex.IsMatch(request.Name, _options.NamingPattern))
        {
            return new SkillForgeResult
            {
                Success = false,
                ErrorMessage = $"Name '{request.Name}' does not match required pattern '{_options.NamingPattern}'."
            };
        }

        var findings = new List<string>();

        if (request.Triggers.Count == 0)
            findings.Add("At least one trigger is required.");

        if (request.Constraints.Count == 0)
            findings.Add("At least one constraint is required.");

        if (string.IsNullOrWhiteSpace(request.Description))
            findings.Add("Description must not be empty.");

        if (_options.RequireGovernanceBar && findings.Count > 0)
        {
            return new SkillForgeResult
            {
                Success = false,
                GovernanceFindings = findings,
                ErrorMessage = "Request does not meet the governance bar."
            };
        }

        var createRequest = new DynamicSkillCreateRequest
        {
            Name = request.Name,
            Description = request.Description,
            Triggers = request.Triggers,
            Constraints = request.Constraints
        };

        var createResult = _engine.Create(createRequest);

        if (!createResult.Success)
        {
            return new SkillForgeResult
            {
                Success = false,
                GovernanceFindings = findings,
                ErrorMessage = createResult.ErrorMessage
            };
        }

        var metadata = new RegistrationMetadata
        {
            ManifestEntry = request.Name,
            DiscoveryKeywords = request.DiscoveryKeywords
        };

        return new SkillForgeResult
        {
            Success = true,
            CreatedSkill = createResult.CreatedSkill,
            GovernanceFindings = findings,
            RegistrationMetadata = metadata
        };
    }
}
