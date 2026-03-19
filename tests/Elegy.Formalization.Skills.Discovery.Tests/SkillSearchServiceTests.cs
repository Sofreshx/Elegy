using Xunit;
using Elegy.Formalization.Skills.Discovery;

namespace Elegy.Formalization.Skills.Discovery.Tests;

public sealed class SkillSearchServiceTests
{
    private readonly SkillSearchService _sut = new();

    private static SkillDiscoveryIndex CreateTestIndex() => new()
    {
        SchemaVersion = 1,
        Entries =
        [
            new SkillIndexEntry
            {
                SkillId = "dotnet-api",
                Name = "dotnet-api",
                Description = "Build .NET APIs with ASP.NET Core",
                TriggersOn = ["dotnet", "api", "aspnet"],
                Keywords = ["backend"],
                Manifest = new SkillIndexManifest
                {
                    Id = "dotnet-api",
                    LoadMode = SkillLoadMode.OnDemand,
                    VaultRef = "dotnet-api/SKILL.md"
                }
            },
            new SkillIndexEntry
            {
                SkillId = "react-frontend",
                Name = "react-frontend",
                Description = "React component patterns and hooks",
                TriggersOn = ["react", "frontend", "hooks"],
                CapabilityHints = ["ui"],
                Manifest = new SkillIndexManifest
                {
                    Id = "react-frontend",
                    LoadMode = SkillLoadMode.OnDemand,
                    VaultRef = "react-frontend/SKILL.md"
                }
            },
            new SkillIndexEntry
            {
                SkillId = "skill-forge",
                Name = "skill-forge",
                Description = "Create new skills programmatically",
                TriggersOn = ["create skill", "forge", "new skill"],
                Keywords = ["generator"],
                Manifest = new SkillIndexManifest
                {
                    Id = "skill-forge",
                    LoadMode = SkillLoadMode.OnDemand,
                    VaultRef = "skill-forge/SKILL.md"
                }
            }
        ],
        BuiltAt = DateTimeOffset.UtcNow
    };

    [Fact]
    public void Search_ExactName_Returns100Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "dotnet-api");

        Assert.Single(results, r => r.Score == 100);
        Assert.Equal("exact-name", results[0].MatchReason);
    }

    [Fact]
    public void Search_PartialName_Returns50Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "react");

        Assert.Contains(results, r => r.Entry.SkillId == "react-frontend" && r.Score == 50);
    }

    [Fact]
    public void Search_TriggerMatch_Returns30Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "hooks");

        Assert.Contains(results, r => r.Entry.SkillId == "react-frontend" && r.Score == 30);
        Assert.Equal("trigger-contains", results.First(r => r.Entry.SkillId == "react-frontend").MatchReason);
    }

    [Fact]
    public void Search_DescriptionMatch_Returns10Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "programmatically");

        Assert.Contains(results, r => r.Entry.SkillId == "skill-forge" && r.Score == 10);
    }

    [Fact]
    public void Search_EmptyQuery_ReturnsAllWithScore0()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "");

        Assert.Equal(3, results.Count);
        Assert.All(results, r => Assert.Equal(0, r.Score));
    }

    [Fact]
    public void Search_MultiWord_MatchesTrigger()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "create skill");

        Assert.Contains(results, r => r.Entry.SkillId == "skill-forge");
    }

    [Fact]
    public void Search_KeywordMatch_Returns20Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "generator");

        Assert.Contains(results, r => r.Entry.SkillId == "skill-forge" && r.Score == 20);
    }

    [Fact]
    public void SearchByStack_DeduplicatesResults()
    {
        var index = CreateTestIndex();
        var results = _sut.SearchByStack(index, ["dotnet", "api"]);

        // "dotnet" and "api" both match dotnet-api; should appear only once
        Assert.Single(results, r => r.Entry.SkillId == "dotnet-api");
    }
}
