using Xunit;
using Elegy.Formalization.Skills.Discovery;

namespace Elegy.Formalization.Skills.Discovery.Tests;

public sealed class SkillIndexBuilderTests
{
    private readonly SkillIndexBuilder _sut = new();

    [Fact]
    public void Build_ValidVault_ReturnsEntries()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-build-{Guid.NewGuid():N}");
        CreateSkillDir(tempDir, "alpha-skill", "---\ndescription: Alpha skill\ntriggersOn: alpha, testing\n---\n# Alpha");
        CreateSkillDir(tempDir, "beta-skill", "---\ndescription: Beta skill\ntriggersOn: beta\n---\n# Beta");

        try
        {
            var index = _sut.Build(tempDir);

            Assert.Equal(1, index.SchemaVersion);
            Assert.Equal(2, index.Entries.Count);
            Assert.Equal("alpha-skill", index.Entries[0].Name); // sorted alpha first
            Assert.Equal("beta-skill", index.Entries[1].Name);
            Assert.Equal("Alpha skill", index.Entries[0].Description);
            Assert.Contains("alpha", index.Entries[0].TriggersOn);
            Assert.Contains("testing", index.Entries[0].TriggersOn);
            Assert.Equal("alpha-skill/SKILL.md", index.Entries[0].Manifest.VaultRef);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Build_EmptyVault_ReturnsEmptyIndex()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-empty-{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);

        try
        {
            var index = _sut.Build(tempDir);

            Assert.Empty(index.Entries);
            Assert.Equal(1, index.SchemaVersion);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Build_NoFrontmatter_StillCreatesEntry()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-nofm-{Guid.NewGuid():N}");
        CreateSkillDir(tempDir, "bare-skill", "# No frontmatter here\nJust content.");

        try
        {
            var index = _sut.Build(tempDir);

            Assert.Single(index.Entries);
            Assert.Equal(string.Empty, index.Entries[0].Description);
            Assert.Empty(index.Entries[0].TriggersOn);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Build_SkipsHiddenDirectories()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-hidden-{Guid.NewGuid():N}");
        CreateSkillDir(tempDir, "visible-skill", "---\ndescription: Visible\n---\n# Visible");
        CreateSkillDir(tempDir, ".hidden-skill", "---\ndescription: Hidden\n---\n# Hidden");

        try
        {
            var index = _sut.Build(tempDir);

            Assert.Single(index.Entries);
            Assert.Equal("visible-skill", index.Entries[0].Name);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    private static void CreateSkillDir(string root, string name, string content)
    {
        var dir = Path.Combine(root, name);
        Directory.CreateDirectory(dir);
        File.WriteAllText(Path.Combine(dir, "SKILL.md"), content);
    }
}
