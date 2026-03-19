using Xunit;
using Elegy.Formalization.Skills.Discovery;

namespace Elegy.Formalization.Skills.Discovery.Tests;

public sealed class SkillDiscoveryIntegrationTests : IDisposable
{
    private readonly string _tempVaultRoot;

    public SkillDiscoveryIntegrationTests()
    {
        _tempVaultRoot = Path.Combine(Path.GetTempPath(), $"elegy-discovery-test-{Guid.NewGuid():N}");
        Directory.CreateDirectory(_tempVaultRoot);
    }

    public void Dispose()
    {
        if (Directory.Exists(_tempVaultRoot))
        {
            Directory.Delete(_tempVaultRoot, recursive: true);
        }
    }

    [Fact]
    public void FullPipeline_BuildSearchResolve_ReturnsContent()
    {
        // Arrange: create 3 skill directories with SKILL.md
        CreateSkill("code-review", "---\ndescription: Reviews code for quality\ntriggersOn: review, quality\n---\n# Code Review\nReviews code.");
        CreateSkill("deploy-helper", "---\ndescription: Helps with deployment\ntriggersOn: deploy, ci\n---\n# Deploy Helper\nDeploys things.");
        CreateSkill("test-runner", "---\ndescription: Runs unit tests\ntriggersOn: test, unit\n---\n# Test Runner\nRuns tests.");

        var builder = new SkillIndexBuilder();
        var searchService = new SkillSearchService();
        var resolveService = new SkillResolveService();

        // Act: build index
        var index = builder.Build(_tempVaultRoot);

        // Assert: 3 entries sorted alphabetically
        Assert.Equal(3, index.Entries.Count);
        Assert.Equal("code-review", index.Entries[0].Name);
        Assert.Equal("deploy-helper", index.Entries[1].Name);
        Assert.Equal("test-runner", index.Entries[2].Name);

        // Act: search by keyword
        var results = searchService.Search(index, "deploy");
        Assert.Single(results);
        Assert.Equal("deploy-helper", results[0].Entry.Name);
        Assert.True(results[0].Score > 0);

        // Act: resolve top result
        var resolveResult = resolveService.Resolve(results[0].Entry, _tempVaultRoot);
        var success = Assert.IsType<SkillResolveResult.Success>(resolveResult);
        Assert.Contains("Deploys things.", success.Content);
    }

    [Fact]
    public void SearchByStack_ReturnsMultipleMatches()
    {
        CreateSkill("code-review", "---\ndescription: Reviews code for quality\ntriggersOn: review, quality\n---\n# Code Review");
        CreateSkill("deploy-helper", "---\ndescription: Helps with deployment\ntriggersOn: deploy, ci\n---\n# Deploy Helper");
        CreateSkill("test-runner", "---\ndescription: Runs unit tests\ntriggersOn: test, unit\n---\n# Test Runner");

        var builder = new SkillIndexBuilder();
        var searchService = new SkillSearchService();

        var index = builder.Build(_tempVaultRoot);
        var results = searchService.SearchByStack(index, ["review", "test"]);

        Assert.Equal(2, results.Count);
        var names = results.Select(r => r.Entry.Name).ToHashSet();
        Assert.Contains("code-review", names);
        Assert.Contains("test-runner", names);
    }

    [Fact]
    public void Resolve_PathTraversal_ReturnsFailure()
    {
        CreateSkill("legit-skill", "---\ndescription: Legit\n---\n# Legit");

        var builder = new SkillIndexBuilder();
        var searchService = new SkillSearchService();
        var resolveService = new SkillResolveService();

        var index = builder.Build(_tempVaultRoot);

        // Craft a malicious entry with traversal VaultRef
        var maliciousEntry = new SkillIndexEntry
        {
            SkillId = "evil",
            Name = "evil",
            Description = string.Empty,
            TriggersOn = [],
            Manifest = new SkillIndexManifest
            {
                Id = "evil",
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = "../../etc/passwd"
            }
        };

        var result = resolveService.Resolve(maliciousEntry, _tempVaultRoot);
        var failure = Assert.IsType<SkillResolveResult.Failure>(result);
        Assert.Equal(SkillResolveError.PathTraversal, failure.Error);
    }

    [Fact]
    public void Resolve_NoVaultRef_ReturnsNotFound()
    {
        var resolveService = new SkillResolveService();

        var entry = new SkillIndexEntry
        {
            SkillId = "missing",
            Name = "missing",
            Description = string.Empty,
            TriggersOn = [],
            Manifest = new SkillIndexManifest
            {
                Id = "missing",
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = null
            }
        };

        var result = resolveService.Resolve(entry, _tempVaultRoot);
        var failure = Assert.IsType<SkillResolveResult.Failure>(result);
        Assert.Equal(SkillResolveError.NotFound, failure.Error);
    }

    [Fact]
    public void Build_SkipsHiddenDirectories()
    {
        CreateSkill("visible-skill", "---\ndescription: Visible\n---\n# Visible");
        // Create hidden directory
        var hiddenDir = Path.Combine(_tempVaultRoot, ".hidden-skill");
        Directory.CreateDirectory(hiddenDir);
        File.WriteAllText(Path.Combine(hiddenDir, "SKILL.md"), "---\ndescription: Hidden\n---\n# Hidden");

        var builder = new SkillIndexBuilder();
        var index = builder.Build(_tempVaultRoot);

        Assert.Single(index.Entries);
        Assert.Equal("visible-skill", index.Entries[0].Name);
    }

    private void CreateSkill(string name, string content)
    {
        var dir = Path.Combine(_tempVaultRoot, name);
        Directory.CreateDirectory(dir);
        File.WriteAllText(Path.Combine(dir, "SKILL.md"), content);
    }
}
