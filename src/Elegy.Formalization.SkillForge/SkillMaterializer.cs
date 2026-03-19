using System.Text;
using Elegy.Formalization.Skills;

namespace Elegy.Formalization.SkillForge;

public sealed class SkillMaterializer
{
    private readonly SkillMaterializerOptions _options;

    public SkillMaterializer(SkillMaterializerOptions options)
    {
        ArgumentNullException.ThrowIfNull(options);
        _options = options;
    }

    public SkillMaterializeResult Materialize(SkillForgeResult result)
    {
        ArgumentNullException.ThrowIfNull(result);

        if (!result.Success || result.CreatedSkill is null)
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = "Cannot materialize a failed forge result."
            };
        }

        var skill = result.CreatedSkill;

        if (string.IsNullOrWhiteSpace(skill.EffectiveId))
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = "Skill identifier is empty."
            };
        }

        if (skill.EffectiveId.Contains("..") || skill.EffectiveId.Contains('/') || skill.EffectiveId.Contains('\\'))
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = "Skill identifier contains invalid path characters."
            };
        }

        var vaultRoot = Path.GetFullPath(_options.VaultRootPath);
        var skillDir = Path.GetFullPath(Path.Combine(vaultRoot, skill.EffectiveId));

        if (!skillDir.StartsWith(vaultRoot, StringComparison.OrdinalIgnoreCase))
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = "Resolved path escapes the vault root."
            };
        }

        var skillFilePath = Path.Combine(skillDir, "SKILL.md");

        if (File.Exists(skillFilePath))
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = $"SKILL.md already exists at '{skillFilePath}'."
            };
        }

        Directory.CreateDirectory(skillDir);
        File.WriteAllText(skillFilePath, GenerateContent(skill), Encoding.UTF8);

        return new SkillMaterializeResult
        {
            Success = true,
            WrittenPath = skillFilePath
        };
    }

    private static string GenerateContent(SkillDefinition skill)
    {
        var sb = new StringBuilder();
        var summary = skill.Metadata.Summary ?? skill.Description ?? string.Empty;

        // YAML frontmatter
        sb.AppendLine("---");
        sb.Append("name: ").AppendLine(skill.EffectiveName);
        sb.Append("description: ").AppendLine(summary);
        sb.Append("triggersOn: ").AppendLine(
            string.Join(", ", skill.Triggers.Select(t => t.Pattern)));
        if (skill.Discovery.Keywords.Count > 0)
            sb.Append("keywords: ").AppendLine(string.Join(", ", skill.Discovery.Keywords));
        if (skill.Discovery.CapabilityHints.Count > 0)
            sb.Append("capabilityHints: ").AppendLine(string.Join(", ", skill.Discovery.CapabilityHints));
        sb.Append("lifecycleState: ").AppendLine(skill.LifecycleState.ToString().ToLowerInvariant());
        sb.AppendLine("---");
        sb.AppendLine();

        // Body
        sb.Append("# ").AppendLine(skill.EffectiveName);
        sb.AppendLine();

        sb.AppendLine("## Purpose");
        sb.AppendLine();
        sb.AppendLine(string.IsNullOrWhiteSpace(summary) ? "No description provided." : summary);
        sb.AppendLine();

        sb.AppendLine("## When to Use");
        sb.AppendLine();
        if (skill.Triggers.Count > 0)
        {
            foreach (var trigger in skill.Triggers)
            {
                sb.Append("- **").Append(trigger.Pattern).Append("**");
                if (!string.IsNullOrWhiteSpace(trigger.Description))
                    sb.Append(": ").Append(trigger.Description);
                sb.AppendLine();
            }
        }
        else
        {
            sb.AppendLine("No triggers defined.");
        }
        sb.AppendLine();

        sb.AppendLine("## Inputs");
        sb.AppendLine();
        if (skill.Input.Parameters.Count > 0)
        {
            foreach (var parameter in skill.Input.Parameters)
            {
                sb.Append("- `").Append(parameter.Name).Append("` (").Append(parameter.Type).Append(')');
                if (!parameter.Required)
                    sb.Append(" optional");
                if (!string.IsNullOrWhiteSpace(parameter.Description))
                    sb.Append(": ").Append(parameter.Description);
                sb.AppendLine();
            }
        }
        else if (!string.IsNullOrWhiteSpace(skill.Input.SchemaRef))
        {
            sb.Append("Schema: ").AppendLine(skill.Input.SchemaRef);
        }
        else
        {
            sb.AppendLine("No explicit inputs defined.");
        }
        sb.AppendLine();

        sb.AppendLine("## Governance");
        sb.AppendLine();
        sb.Append("Risk level: ").AppendLine(skill.Governance.RiskLevel.ToString());
        sb.Append("Approval: ").AppendLine(skill.Governance.ApprovalRequirement.ToString());
        if (skill.Governance.PolicyRefs.Count > 0)
            sb.Append("Policies: ").AppendLine(string.Join(", ", skill.Governance.PolicyRefs));
        if (skill.Governance.AllowedContexts.Count > 0)
            sb.Append("Allowed contexts: ").AppendLine(string.Join(", ", skill.Governance.AllowedContexts));
        sb.AppendLine();

        sb.AppendLine("## Constraints");
        sb.AppendLine();
        if (skill.Constraints.Count > 0)
        {
            foreach (var constraint in skill.Constraints)
            {
                sb.Append("- ");
                if (constraint.Required) sb.Append("**(required)** ");
                sb.Append('`').Append(constraint.ConstraintId).Append('`');
                if (!string.IsNullOrWhiteSpace(constraint.Description))
                    sb.Append(": ").Append(constraint.Description);
                sb.AppendLine();
            }
        }
        else
        {
            sb.AppendLine("No constraints defined.");
        }

        return sb.ToString();
    }
}
