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
                Id = "dotnet-api",
                Name = "dotnet-api",
                Description = "Build .NET APIs with ASP.NET Core",
                Triggers = ["dotnet", "api", "aspnet"],
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = "dotnet-api/SKILL.md"
            },
            new SkillIndexEntry
            {
                Id = "react-frontend",
                Name = "react-frontend",
                Description = "React component patterns and hooks",
                Triggers = ["react", "frontend", "hooks"],
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = "react-frontend/SKILL.md"
            },
            new SkillIndexEntry
            {
                Id = "skill-forge",
                Name = "skill-forge",
                Description = "Create new skills programmatically",
                Triggers = ["create skill", "forge", "new skill"],
                LoadMode = SkillLoadMode.OnDemand,
                VaultRef = "skill-forge/SKILL.md"
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

        Assert.Contains(results, r => r.Entry.Id == "react-frontend" && r.Score == 50);
    }

    [Fact]
    public void Search_TriggerMatch_Returns30Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "hooks");

        Assert.Contains(results, r => r.Entry.Id == "react-frontend" && r.Score == 30);
        Assert.Equal("trigger-contains", results.First(r => r.Entry.Id == "react-frontend").MatchReason);
    }

    [Fact]
    public void Search_DescriptionMatch_Returns10Score()
    {
        var index = CreateTestIndex();
        var results = _sut.Search(index, "programmatically");

        Assert.Contains(results, r => r.Entry.Id == "skill-forge" && r.Score == 10);
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

        Assert.Contains(results, r => r.Entry.Id == "skill-forge");
    }

    [Fact]
    public void SearchByStack_DeduplicatesResults()
    {
        var index = CreateTestIndex();
        var results = _sut.SearchByStack(index, ["dotnet", "api"]);

        // "dotnet" and "api" both match dotnet-api; should appear only once
        Assert.Single(results, r => r.Entry.Id == "dotnet-api");
    }
}
