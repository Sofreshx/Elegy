using Xunit;

namespace Elegy.Formalization.Skills.Discovery.Tests;

public class SkillIndexEntryTests
{
    [Fact]
    public void Default_construction_produces_valid_defaults()
    {
        var entry = new SkillIndexEntry();

        Assert.Equal(string.Empty, entry.Id);
        Assert.Equal(string.Empty, entry.SkillId);
        Assert.Equal(string.Empty, entry.Name);
        Assert.Equal(string.Empty, entry.Description);
        Assert.Equal(SkillLifecycleState.Draft, entry.LifecycleState);
        Assert.Empty(entry.Triggers);
        Assert.Empty(entry.Keywords);
        Assert.Empty(entry.CapabilityHints);
        Assert.Equal(SkillLoadMode.Always, entry.LoadMode);
        Assert.NotNull(entry.Manifest);
        Assert.Null(entry.VaultRef);
    }

    [Fact]
    public void Init_properties_set_correctly()
    {
        var entry = new SkillIndexEntry
        {
            SkillId = "skill-001",
            Name = "My Skill",
            Description = "Does things",
            LifecycleState = SkillLifecycleState.Active,
            TriggersOn = ["keyword1", "keyword2"],
            Keywords = ["searchable"],
            CapabilityHints = ["lookup"],
            Manifest = new SkillIndexManifest
            {
                Id = "skill-001",
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = "skills-vault/my-skill",
                SourceKind = SkillSourceKind.Imported,
                MaterializationKind = SkillMaterializationKind.Dynamic,
            },
        };

        Assert.Equal("skill-001", entry.Id);
        Assert.Equal("My Skill", entry.Name);
        Assert.Equal("Does things", entry.Description);
        Assert.Equal(SkillLifecycleState.Active, entry.LifecycleState);
        Assert.Equal(2, entry.Triggers.Count);
        Assert.Equal("keyword1", entry.Triggers[0]);
        Assert.Single(entry.Keywords);
        Assert.Single(entry.CapabilityHints);
        Assert.Equal(SkillLoadMode.OnDemand, entry.LoadMode);
        Assert.Equal("skills-vault/my-skill", entry.VaultRef);
    }

    [Fact]
    public void Entries_list_is_immutable()
    {
        var entry = new SkillIndexEntry { TriggersOn = ["a", "b"] };

        Assert.IsAssignableFrom<IReadOnlyList<string>>(entry.Triggers);
    }

    [Fact]
    public void FromSkillDefinition_Projects_Canonical_Fields()
    {
        var definition = new SkillDefinition
        {
            Id = "skill-001",
            Name = "My Skill",
            Description = "A projected skill",
            Metadata = new SkillMetadata
            {
                Summary = "A projected skill",
            },
            Triggers = [new SkillTrigger { Pattern = "keyword1" }],
            Discovery = new SkillDiscoveryMetadata
            {
                Keywords = ["searchable"],
                CapabilityHints = ["lookup"],
            },
            LifecycleState = SkillLifecycleState.Active,
        };

        var entry = SkillIndexEntry.FromSkillDefinition(
            definition,
            new SkillIndexManifest
            {
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = "skill-001/SKILL.md",
            });

        Assert.Equal("skill-001", entry.SkillId);
        Assert.Equal("My Skill", entry.Name);
        Assert.Equal("A projected skill", entry.Description);
        Assert.Equal(SkillLifecycleState.Active, entry.LifecycleState);
        Assert.Contains("keyword1", entry.TriggersOn);
        Assert.Contains("searchable", entry.Keywords);
        Assert.Contains("lookup", entry.CapabilityHints);
        Assert.Equal("skill-001", entry.Manifest.Id);
    }

    [Fact]
    public void SkillDiscoveryIndex_default_construction()
    {
        var index = new SkillDiscoveryIndex();

        Assert.Equal(1, index.SchemaVersion);
        Assert.Empty(index.Entries);
        Assert.Equal(DateTimeOffset.MinValue, index.BuiltAt);
    }

    [Fact]
    public void SkillDiscoveryIndex_entries_are_readonly()
    {
        var index = new SkillDiscoveryIndex
        {
            Entries = [new SkillIndexEntry { SkillId = "s1" }],
        };

        Assert.IsAssignableFrom<IReadOnlyList<SkillIndexEntry>>(index.Entries);
        Assert.Single(index.Entries);
    }
}
