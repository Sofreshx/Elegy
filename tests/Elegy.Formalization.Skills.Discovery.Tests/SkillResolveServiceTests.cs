using Xunit;
using Elegy.Formalization.Skills.Discovery;

namespace Elegy.Formalization.Skills.Discovery.Tests;

public sealed class SkillResolveServiceTests
{
    private readonly SkillResolveService _sut = new();

    [Fact]
    public void Resolve_ValidFile_ReturnsSuccess()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-test-{Guid.NewGuid():N}");
        var skillDir = Path.Combine(tempDir, "my-skill");
        Directory.CreateDirectory(skillDir);
        File.WriteAllText(Path.Combine(skillDir, "SKILL.md"), "# My Skill\nContent here.");

        try
        {
            var entry = new SkillIndexEntry { Id = "my-skill", VaultRef = "my-skill/SKILL.md" };
            var result = _sut.Resolve(entry, tempDir);

            Assert.IsType<SkillResolveResult.Success>(result);
            var success = (SkillResolveResult.Success)result;
            Assert.Contains("My Skill", success.Content);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Resolve_PathTraversal_ReturnsFailure()
    {
        var entry = new SkillIndexEntry { Id = "evil", VaultRef = "../../etc/passwd" };
        var result = _sut.Resolve(entry, "/tmp/vault");

        Assert.IsType<SkillResolveResult.Failure>(result);
        Assert.Equal(SkillResolveError.PathTraversal, ((SkillResolveResult.Failure)result).Error);
    }

    [Fact]
    public void Resolve_OutsideVault_ReturnsFailure()
    {
        // Even without ".." if the resolved path escapes the vault root
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-test-{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);

        try
        {
            // VaultRef that, after GetFullPath, would be absolute and outside vault
            var entry = new SkillIndexEntry { Id = "evil", VaultRef = Path.GetFullPath("/") };
            var result = _sut.Resolve(entry, tempDir);

            // Should be either PathTraversal or OutsideVault depending on whether ".." is in the ref
            var failure = Assert.IsType<SkillResolveResult.Failure>(result);
            Assert.True(
                failure.Error == SkillResolveError.OutsideVault || failure.Error == SkillResolveError.NotFound,
                $"Expected OutsideVault or NotFound, got {failure.Error}");
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Resolve_MissingFile_ReturnsNotFound()
    {
        var tempDir = Path.Combine(Path.GetTempPath(), $"vault-test-{Guid.NewGuid():N}");
        Directory.CreateDirectory(tempDir);

        try
        {
            var entry = new SkillIndexEntry { Id = "missing", VaultRef = "missing/SKILL.md" };
            var result = _sut.Resolve(entry, tempDir);

            Assert.IsType<SkillResolveResult.Failure>(result);
            Assert.Equal(SkillResolveError.NotFound, ((SkillResolveResult.Failure)result).Error);
        }
        finally
        {
            Directory.Delete(tempDir, true);
        }
    }

    [Fact]
    public void Resolve_NullVaultRef_ReturnsNotFound()
    {
        var entry = new SkillIndexEntry { Id = "no-ref", VaultRef = null };
        var result = _sut.Resolve(entry, "/tmp/vault");

        Assert.IsType<SkillResolveResult.Failure>(result);
        Assert.Equal(SkillResolveError.NotFound, ((SkillResolveResult.Failure)result).Error);
    }

    [Fact]
    public void Resolve_EmptyVaultRef_ReturnsNotFound()
    {
        var entry = new SkillIndexEntry { Id = "empty-ref", VaultRef = "" };
        var result = _sut.Resolve(entry, "/tmp/vault");

        Assert.IsType<SkillResolveResult.Failure>(result);
        Assert.Equal(SkillResolveError.NotFound, ((SkillResolveResult.Failure)result).Error);
    }
}
