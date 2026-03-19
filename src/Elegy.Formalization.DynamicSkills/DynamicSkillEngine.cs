using Elegy.Formalization.Core.Agentic;
using Elegy.Formalization.Skills;
using Elegy.Formalization.Monitoring;

namespace Elegy.Formalization.DynamicSkills;

public sealed class DynamicSkillEngine
{
    private readonly DynamicSkillEngineOptions _options;

    public DynamicSkillEngine(DynamicSkillEngineOptions options)
    {
        _options = options;
    }

    public DynamicSkillCreateResult Create(DynamicSkillCreateRequest request)
    {
        EnsureEnabled();

        if (string.IsNullOrWhiteSpace(request.Name) && string.IsNullOrWhiteSpace(request.Identity.DisplayName))
        {
            return new DynamicSkillCreateResult
            {
                Success = false,
                Validation = new SkillValidationResult
                {
                    Errors = ["Skill name is required."],
                },
                ErrorMessage = "Name is required."
            };
        }

        var skillId = ResolveSkillId(request);
        var displayName = ResolveDisplayName(request, skillId);
        var origin = NormalizeOrigin(request.Origin, skillId);

        var skill = new SkillDefinition
        {
            Id = skillId,
            Name = displayName,
            Description = request.Description,
            Triggers = request.Triggers,
            Constraints = request.Constraints,
            LifecycleState = request.LifecycleState,
            Identity = request.Identity with
            {
                DefinitionId = string.IsNullOrWhiteSpace(request.Identity.DefinitionId) ? skillId : request.Identity.DefinitionId,
                DisplayName = string.IsNullOrWhiteSpace(request.Identity.DisplayName) ? displayName : request.Identity.DisplayName,
            },
            Metadata = request.Metadata with
            {
                Summary = request.Metadata.Summary ?? request.Description,
            },
            Input = request.Input,
            Output = request.Output,
            Execution = request.Execution,
            Governance = request.Governance,
            Discovery = request.Discovery,
            Origin = origin,
        };

        var validation = SkillDefinitionValidator.Validate(skill);

        if (!validation.IsValid)
        {
            return new DynamicSkillCreateResult
            {
                Success = false,
                Validation = validation,
                ErrorMessage = validation.Errors[0],
            };
        }

        return new DynamicSkillCreateResult
        {
            Success = true,
            CreatedSkill = skill,
            Validation = validation,
        };
    }

    public DynamicSkillValidationResult Validate(SkillDefinition definition)
    {
        EnsureEnabled();

        return new DynamicSkillValidationResult
        {
            Validation = SkillDefinitionValidator.Validate(definition)
        };
    }

    public DynamicSkillDeactivateResult Deactivate(string skillId)
    {
        EnsureEnabled();

        if (string.IsNullOrWhiteSpace(skillId))
        {
            return new DynamicSkillDeactivateResult
            {
                Success = false,
                ErrorMessage = "Skill ID is required."
            };
        }

        return new DynamicSkillDeactivateResult
        {
            Success = true
        };
    }

    private void EnsureEnabled()
    {
        if (!_options.IsEnabled)
        {
            throw new InvalidOperationException("DynamicSkillEngine is not enabled. Set IsEnabled to true in DynamicSkillEngineOptions.");
        }
    }

    private static string ResolveSkillId(DynamicSkillCreateRequest request)
    {
        if (!string.IsNullOrWhiteSpace(request.SkillId))
        {
            return request.SkillId;
        }

        if (!string.IsNullOrWhiteSpace(request.Identity.DefinitionId))
        {
            return request.Identity.DefinitionId;
        }

        return Guid.NewGuid().ToString("N");
    }

    private static string ResolveDisplayName(DynamicSkillCreateRequest request, string skillId)
    {
        if (!string.IsNullOrWhiteSpace(request.Name))
        {
            return request.Name;
        }

        if (!string.IsNullOrWhiteSpace(request.Identity.DisplayName))
        {
            return request.Identity.DisplayName;
        }

        return skillId;
    }

    private static SkillOrigin NormalizeOrigin(SkillOrigin origin, string skillId)
    {
        return origin with
        {
            MaterializationKind = SkillMaterializationKind.Dynamic,
            SourceRef = string.IsNullOrWhiteSpace(origin.SourceRef) ? $"dynamic:{skillId}" : origin.SourceRef,
        };
    }
}
