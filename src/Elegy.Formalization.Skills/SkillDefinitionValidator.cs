namespace Elegy.Formalization.Skills;

public static class SkillDefinitionValidator
{
    public static SkillValidationResult Validate(SkillDefinition definition)
    {
        ArgumentNullException.ThrowIfNull(definition);

        var errors = new List<string>();

        if (string.IsNullOrWhiteSpace(definition.EffectiveId))
        {
            errors.Add("Skill definition ID is required.");
        }

        if (string.IsNullOrWhiteSpace(definition.EffectiveName))
        {
            errors.Add("Skill name is required.");
        }

        if (definition.Triggers.Any(static trigger => string.IsNullOrWhiteSpace(trigger.Pattern)))
        {
            errors.Add("Skill triggers must define a non-empty pattern.");
        }

        if (definition.Constraints.Any(static constraint => string.IsNullOrWhiteSpace(constraint.ConstraintId)))
        {
            errors.Add("Skill constraints must define a non-empty constraint ID.");
        }

        if (definition.Identity.Aliases.Any(static alias => string.IsNullOrWhiteSpace(alias)))
        {
            errors.Add("Skill identity aliases must not be blank.");
        }

        if (HasDuplicateValues(definition.Identity.Aliases))
        {
            errors.Add("Skill identity aliases must be unique.");
        }

        if (definition.Metadata.Tags.Any(static tag => string.IsNullOrWhiteSpace(tag)))
        {
            errors.Add("Skill metadata tags must not be blank.");
        }

        if (definition.Metadata.Owners.Any(static owner => string.IsNullOrWhiteSpace(owner)))
        {
            errors.Add("Skill metadata owners must not be blank.");
        }

        if (definition.Input.Parameters.Any(static parameter => string.IsNullOrWhiteSpace(parameter.Name)))
        {
            errors.Add("Skill input parameters must define a non-empty name.");
        }

        if (HasDuplicateValues(definition.Input.Parameters.Select(static parameter => parameter.Name)))
        {
            errors.Add("Skill input parameter names must be unique.");
        }

        if (definition.Input.Parameters.Any(static parameter => string.IsNullOrWhiteSpace(parameter.Type)))
        {
            errors.Add("Skill input parameters must define a non-empty type.");
        }

        if (definition.Execution.TimeoutSeconds is <= 0)
        {
            errors.Add("Skill execution timeout, when provided, must be greater than zero seconds.");
        }

        if (definition.Governance.ApprovalRequirement != SkillApprovalRequirement.None && definition.Governance.PolicyRefs.Count == 0)
        {
            errors.Add("Skills that require approval must declare at least one policy reference.");
        }

        if (definition.Governance.PolicyRefs.Any(static policyRef => string.IsNullOrWhiteSpace(policyRef)))
        {
            errors.Add("Skill governance policy references must not be blank.");
        }

        if (definition.Governance.AllowedContexts.Any(static context => string.IsNullOrWhiteSpace(context)))
        {
            errors.Add("Skill governance allowed contexts must not be blank.");
        }

        if (definition.Discovery.Keywords.Any(static keyword => string.IsNullOrWhiteSpace(keyword)))
        {
            errors.Add("Skill discovery keywords must not be blank.");
        }

        if (definition.Discovery.CapabilityHints.Any(static hint => string.IsNullOrWhiteSpace(hint)))
        {
            errors.Add("Skill discovery capability hints must not be blank.");
        }

        if (definition.Origin.MaterializationKind == SkillMaterializationKind.Dynamic &&
            definition.Origin.SourceKind == SkillSourceKind.Manual &&
            string.IsNullOrWhiteSpace(definition.Origin.SourceRef))
        {
            errors.Add("Dynamic skills must declare either a source reference or a non-manual source kind.");
        }

        return new SkillValidationResult
        {
            Errors = errors,
        };
    }

    private static bool HasDuplicateValues(IEnumerable<string> values)
    {
        var distinct = new HashSet<string>(StringComparer.OrdinalIgnoreCase);

        foreach (var value in values)
        {
            if (string.IsNullOrWhiteSpace(value))
            {
                continue;
            }

            if (!distinct.Add(value))
            {
                return true;
            }
        }

        return false;
    }
}