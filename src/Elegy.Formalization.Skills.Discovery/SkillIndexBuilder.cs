using System.Text.RegularExpressions;

namespace Elegy.Formalization.Skills.Discovery;

public sealed class SkillIndexBuilder
{
    private static readonly Regex FrontmatterPattern = new(
        @"^---\s*\n(.*?)\n---",
        RegexOptions.Singleline | RegexOptions.Compiled);

    public SkillDiscoveryIndex Build(string vaultRootPath)
    {
        var entries = new List<SkillIndexEntry>();
        var rootDir = new DirectoryInfo(vaultRootPath);

        if (!rootDir.Exists)
        {
            return new SkillDiscoveryIndex
            {
                SchemaVersion = 1,
                Entries = [],
                BuiltAt = DateTimeOffset.UtcNow
            };
        }

        foreach (var subDir in rootDir.EnumerateDirectories())
        {
            // Skip hidden directories
            if (subDir.Name.StartsWith('.'))
                continue;

            var skillFile = Path.Combine(subDir.FullName, "SKILL.md");
            if (!File.Exists(skillFile))
                continue;

            var content = File.ReadAllText(skillFile);
            var entry = ParseSkillFile(subDir.Name, content);
            entries.Add(entry);
        }

        entries.Sort((a, b) => string.Compare(a.Name, b.Name, StringComparison.OrdinalIgnoreCase));

        return new SkillDiscoveryIndex
        {
            SchemaVersion = 1,
            Entries = entries,
            BuiltAt = DateTimeOffset.UtcNow
        };
    }

    private static SkillIndexEntry ParseSkillFile(string directoryName, string content)
    {
        var description = (string?)null;
        var triggers = new List<string>();

        var match = FrontmatterPattern.Match(content);
        if (match.Success)
        {
            var yaml = match.Groups[1].Value;
            description = ExtractYamlValue(yaml, "description");

            var triggersOn = ExtractYamlValue(yaml, "triggersOn");
            if (triggersOn is not null)
            {
                // Simple parsing: comma-separated or YAML list items
                triggers.AddRange(
                    triggersOn.Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries));
            }
        }

        return new SkillIndexEntry
        {
            Id = directoryName,
            Name = directoryName,
            Description = description,
            Triggers = triggers,
            LoadMode = SkillLoadMode.OnDemand,
            VaultRef = $"{directoryName}/SKILL.md"
        };
    }

    private static string? ExtractYamlValue(string yaml, string key)
    {
        foreach (var line in yaml.Split('\n'))
        {
            var trimmed = line.TrimStart();
            if (trimmed.StartsWith($"{key}:", StringComparison.OrdinalIgnoreCase))
            {
                var value = trimmed[($"{key}:".Length)..].Trim().Trim('"', '\'');
                return string.IsNullOrWhiteSpace(value) ? null : value;
            }
        }
        return null;
    }
}
