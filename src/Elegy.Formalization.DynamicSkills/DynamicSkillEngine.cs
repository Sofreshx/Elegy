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

        if (string.IsNullOrWhiteSpace(request.Name))
        {
            return new DynamicSkillCreateResult
            {
                Success = false,
                ErrorMessage = "Name is required."
            };
        }

        var skill = new SkillDefinition
        {
            Id = Guid.NewGuid().ToString("N"),
            Name = request.Name,
            Description = request.Description,
            Triggers = request.Triggers,
            Constraints = request.Constraints,
            LifecycleState = SkillLifecycleState.Draft
        };

        return new DynamicSkillCreateResult
        {
            Success = true,
            CreatedSkill = skill
        };
    }

    public DynamicSkillValidationResult Validate(SkillDefinition definition)
    {
        EnsureEnabled();

        var errors = new List<string>();

        if (string.IsNullOrWhiteSpace(definition.Name))
        {
            errors.Add("Name is required.");
        }

        return new DynamicSkillValidationResult
        {
            IsValid = errors.Count == 0,
            Errors = errors
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
}
