using Xunit;

namespace Elegy.Formalization.Skills.Discovery.Tests;

public class SkillIndexEntryTests
{
    [Fact]
    public void Default_construction_produces_valid_defaults()
    {
        var entry = new SkillIndexEntry();

        Assert.Equal(string.Empty, entry.Id);
        Assert.Equal(string.Empty, entry.Name);
        Assert.Null(entry.Description);
        Assert.Empty(entry.Triggers);
        Assert.Equal(SkillLoadMode.Always, entry.LoadMode);
        Assert.Null(entry.VaultRef);
    }

    [Fact]
    public void Init_properties_set_correctly()
    {
        var entry = new SkillIndexEntry
        {
            Id = "skill-001",
            Name = "My Skill",
            Description = "Does things",
            Triggers = ["keyword1", "keyword2"],
            LoadMode = SkillLoadMode.OnDemand,
            VaultRef = "skills-vault/my-skill",
        };

        Assert.Equal("skill-001", entry.Id);
        Assert.Equal("My Skill", entry.Name);
        Assert.Equal("Does things", entry.Description);
        Assert.Equal(2, entry.Triggers.Count);
        Assert.Equal("keyword1", entry.Triggers[0]);
        Assert.Equal(SkillLoadMode.OnDemand, entry.LoadMode);
        Assert.Equal("skills-vault/my-skill", entry.VaultRef);
    }

    [Fact]
    public void Entries_list_is_immutable()
    {
        var entry = new SkillIndexEntry { Triggers = ["a", "b"] };

        Assert.IsAssignableFrom<IReadOnlyList<string>>(entry.Triggers);
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
            Entries = [new SkillIndexEntry { Id = "s1" }],
        };

        Assert.IsAssignableFrom<IReadOnlyList<SkillIndexEntry>>(index.Entries);
        Assert.Single(index.Entries);
    }
}
