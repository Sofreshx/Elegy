using System.Text.RegularExpressions;
using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

[Obsolete("Compatibility surface only. Prefer canonical skill authority, governed artifacts, or Rust tooling for shared executable behavior.")]
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
        var requestedId = ResolveRequestedId(request);

        if (!Regex.IsMatch(requestedId, _options.NamingPattern))
        {
            return new SkillForgeResult
            {
                Success = false,
                ErrorMessage = $"Name '{requestedId}' does not match required pattern '{_options.NamingPattern}'."
            };
        }

        var findings = new List<string>();

        if (request.Triggers.Count == 0)
            findings.Add("At least one trigger is required.");

        if (request.Constraints.Count == 0)
            findings.Add("At least one constraint is required.");

        if (string.IsNullOrWhiteSpace(request.Description) && string.IsNullOrWhiteSpace(request.Metadata.Summary))
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
            SkillId = requestedId,
            Name = request.Name,
            Description = request.Description,
            Triggers = request.Triggers,
            Constraints = request.Constraints,
            Identity = request.Identity with
            {
                DefinitionId = string.IsNullOrWhiteSpace(request.Identity.DefinitionId) ? requestedId : request.Identity.DefinitionId,
                DisplayName = string.IsNullOrWhiteSpace(request.Identity.DisplayName) ? request.Name : request.Identity.DisplayName,
            },
            Metadata = request.Metadata with
            {
                Summary = request.Metadata.Summary ?? request.Description,
            },
            Input = request.Input,
            Output = request.Output,
            Execution = request.Execution,
            Governance = request.Governance,
            Discovery = request.Discovery with
            {
                Keywords = request.Discovery.Keywords
                    .Concat(request.DiscoveryKeywords)
                    .Where(static keyword => !string.IsNullOrWhiteSpace(keyword))
                    .Distinct(StringComparer.OrdinalIgnoreCase)
                    .ToArray(),
            },
            Origin = request.Origin with
            {
                SourceRef = string.IsNullOrWhiteSpace(request.Origin.SourceRef) ? $"forge:{requestedId}" : request.Origin.SourceRef,
            }
        };

        var createResult = _engine.Create(createRequest);

        if (!createResult.Success)
        {
            return new SkillForgeResult
            {
                Success = false,
                Validation = createResult.Validation,
                GovernanceFindings = findings,
                ErrorMessage = createResult.ErrorMessage
            };
        }

        var metadata = new RegistrationMetadata
        {
            ManifestEntry = requestedId,
            SkillId = createResult.CreatedSkill!.EffectiveId,
            DiscoveryKeywords = createResult.CreatedSkill.Discovery.Keywords,
            SourceKind = createResult.CreatedSkill.Origin.SourceKind,
            MaterializationKind = createResult.CreatedSkill.Origin.MaterializationKind,
        };

        return new SkillForgeResult
        {
            Success = true,
            CreatedSkill = createResult.CreatedSkill,
            Validation = createResult.Validation,
            GovernanceFindings = findings,
            RegistrationMetadata = metadata
        };
    }

    private static string ResolveRequestedId(SkillForgeRequest request)
    {
        if (!string.IsNullOrWhiteSpace(request.SkillId))
        {
            return request.SkillId;
        }

        if (!string.IsNullOrWhiteSpace(request.Identity.DefinitionId))
        {
            return request.Identity.DefinitionId;
        }

        return request.Name;
    }
}
