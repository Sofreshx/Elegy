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

        if (string.IsNullOrWhiteSpace(skill.Name))
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = "Skill name is empty."
            };
        }

        if (skill.Name.Contains("..") || skill.Name.Contains('/') || skill.Name.Contains('\\'))
        {
            return new SkillMaterializeResult
            {
                Success = false,
                ErrorMessage = "Skill name contains invalid path characters."
            };
        }

        var vaultRoot = Path.GetFullPath(_options.VaultRootPath);
        var skillDir = Path.GetFullPath(Path.Combine(vaultRoot, skill.Name));

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

        // YAML frontmatter
        sb.AppendLine("---");
        sb.Append("name: ").AppendLine(skill.Name);
        sb.Append("description: ").AppendLine(skill.Description ?? string.Empty);
        sb.Append("triggersOn: ").AppendLine(
            string.Join(", ", skill.Triggers.Select(t => t.Pattern)));
        sb.AppendLine("---");
        sb.AppendLine();

        // Body
        sb.Append("# ").AppendLine(skill.Name);
        sb.AppendLine();

        sb.AppendLine("## Purpose");
        sb.AppendLine();
        sb.AppendLine(skill.Description ?? "No description provided.");
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
