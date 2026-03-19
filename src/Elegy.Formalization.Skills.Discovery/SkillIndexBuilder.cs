using System.Text.RegularExpressions;
using Elegy.Formalization.Skills;

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
        var name = directoryName;
        var description = (string?)null;
        var triggers = Array.Empty<string>();
        var keywords = Array.Empty<string>();
        var capabilityHints = Array.Empty<string>();
        var loadMode = SkillLoadMode.OnDemand;
        var lifecycleState = SkillLifecycleState.Draft;

        var match = FrontmatterPattern.Match(content);
        if (match.Success)
        {
            var yaml = match.Groups[1].Value;
            name = ExtractYamlValue(yaml, "name") ?? directoryName;
            description = ExtractYamlValue(yaml, "description");
            triggers = ParseCsvList(ExtractYamlValue(yaml, "triggersOn"));
            keywords = ParseCsvList(ExtractYamlValue(yaml, "keywords"));
            capabilityHints = ParseCsvList(ExtractYamlValue(yaml, "capabilityHints"));
            loadMode = ParseLoadMode(ExtractYamlValue(yaml, "loadMode"));
            lifecycleState = ParseLifecycleState(ExtractYamlValue(yaml, "lifecycleState"));
        }

        var skill = new SkillDefinition
        {
            Id = directoryName,
            Name = name,
            Description = description,
            Metadata = new SkillMetadata
            {
                Summary = description,
            },
            Triggers = triggers.Select(static trigger => new SkillTrigger { Pattern = trigger }).ToArray(),
            Discovery = new SkillDiscoveryMetadata
            {
                Keywords = keywords,
                CapabilityHints = capabilityHints,
            },
            LifecycleState = lifecycleState,
            Origin = new SkillOrigin
            {
                MaterializationKind = SkillMaterializationKind.Declared,
                SourceKind = SkillSourceKind.Manual,
                SourceRef = $"{directoryName}/SKILL.md",
            },
        };

        return SkillIndexEntry.FromSkillDefinition(
            skill,
            new SkillIndexManifest
            {
                Id = directoryName,
                LoadMode = loadMode,
                VaultRef = $"{directoryName}/SKILL.md",
                SourceKind = SkillSourceKind.Manual,
                MaterializationKind = SkillMaterializationKind.Declared,
            });
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

    private static string[] ParseCsvList(string? value)
    {
        if (string.IsNullOrWhiteSpace(value))
        {
            return [];
        }

        return value
            .Split(',', StringSplitOptions.RemoveEmptyEntries | StringSplitOptions.TrimEntries)
            .Where(static item => !string.IsNullOrWhiteSpace(item))
            .ToArray();
    }

    private static SkillLoadMode ParseLoadMode(string? value)
    {
        if (string.Equals(value, "always", StringComparison.OrdinalIgnoreCase))
        {
            return SkillLoadMode.Always;
        }

        return SkillLoadMode.OnDemand;
    }

    private static SkillLifecycleState ParseLifecycleState(string? value)
    {
        if (Enum.TryParse<SkillLifecycleState>(value, ignoreCase: true, out var lifecycleState))
        {
            return lifecycleState;
        }

        return SkillLifecycleState.Draft;
    }
}
