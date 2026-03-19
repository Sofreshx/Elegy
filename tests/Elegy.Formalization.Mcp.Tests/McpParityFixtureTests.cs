using System.IO;
using System.Linq;
using Elegy.Formalization.Serialization;
using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.Mcp.Tests;

public sealed class McpParityFixtureTests
{
    [Fact]
    public void Analyze_shared_fixture_matches_expected_analysis_golden()
    {
        var descriptor = ReadCanonical<McpServerDescriptor>(AuthoritativeFixturePath("mcp-server-descriptor.parity.json"));
        var expected = ReadCanonical<McpAnalysisResult>(AuthoritativeFixturePath("mcp-analysis-result.parity.json"));

        var actual = new McpToolAnalyzer().Analyze(descriptor);

        Assert.Equal(CanonicalJsonSerializer.Serialize(expected), CanonicalJsonSerializer.Serialize(actual));
    }

    [Fact]
    public void Generate_and_discovery_services_match_shared_parity_golden()
    {
        var descriptor = ReadCanonical<McpServerDescriptor>(AuthoritativeFixturePath("mcp-server-descriptor.parity.json"));
        var expected = ReadCanonical<McpParityExpectation>(AuthoritativeFixturePath("mcp-parity-expected.json"));

        var analysis = new McpToolAnalyzer().Analyze(descriptor);
        var generation = new McpSkillGenerator().Generate(analysis);
        var searchResults = new McpToolSearchService().Search(descriptor, expected.Search.Query);
        var resolved = new McpToolResolveService().Resolve(descriptor, expected.Resolve.ToolName);

        Assert.NotNull(resolved);

        var actual = new McpParityExpectation
        {
            GeneratedSkills = generation.GeneratedSkills,
            SkippedToolNames = generation.SkippedTools.Select(static tool => tool.Name).ToArray(),
            Search = new McpSearchExpectation
            {
                Query = expected.Search.Query,
                Results = searchResults
            },
            Resolve = new McpResolveExpectation
            {
                ToolName = expected.Resolve.ToolName,
                Result = resolved!
            }
        };

        Assert.Equal(CanonicalJsonSerializer.Serialize(expected), CanonicalJsonSerializer.Serialize(actual));
    }

    [Fact]
    public void Shared_parity_fixtures_are_mirrored_to_artifacts_contracts()
    {
        var sharedFiles = new[]
        {
            "mcp-server-descriptor.parity.json",
            "mcp-analysis-result.parity.json",
            "mcp-parity-expected.json"
        };

        foreach (var fileName in sharedFiles)
        {
            var authoritative = NormalizeLineEndings(File.ReadAllText(AuthoritativeFixturePath(fileName)));
            var artifact = NormalizeLineEndings(File.ReadAllText(ArtifactFixturePath(fileName)));
            Assert.Equal(authoritative, artifact);
        }
    }

    private static T ReadCanonical<T>(string path)
    {
        return CanonicalJsonSerializer.Deserialize<T>(File.ReadAllText(path));
    }

    private static string AuthoritativeFixturePath(string fileName)
    {
        return Path.Combine(RepoRoot, "src", "Elegy.Formalization.Contracts", "Resources", "fixtures", fileName);
    }

    private static string ArtifactFixturePath(string fileName)
    {
        return Path.Combine(RepoRoot, "artifacts", "contracts", "fixtures", fileName);
    }

    private static string RepoRoot => FindRepoRoot();

    private static string FindRepoRoot()
    {
        var directory = new DirectoryInfo(AppContext.BaseDirectory);
        while (directory is not null && !File.Exists(Path.Combine(directory.FullName, "Elegy.sln")))
        {
            directory = directory.Parent;
        }

        if (directory is null)
        {
            throw new DirectoryNotFoundException("Could not locate the Elegy repository root.");
        }

        return directory.FullName;
    }

    private static string NormalizeLineEndings(string content)
    {
        return content.Replace("\r\n", "\n");
    }

    private sealed record McpParityExpectation
    {
        public IReadOnlyList<SkillDefinition> GeneratedSkills { get; init; } = [];
        public IReadOnlyList<string> SkippedToolNames { get; init; } = [];
        public McpSearchExpectation Search { get; init; } = new();
        public McpResolveExpectation Resolve { get; init; } = new();
    }

    private sealed record McpSearchExpectation
    {
        public string Query { get; init; } = string.Empty;
        public IReadOnlyList<McpToolSummary> Results { get; init; } = [];
    }

    private sealed record McpResolveExpectation
    {
        public string ToolName { get; init; } = string.Empty;
        public McpToolDefinition Result { get; init; } = new();
    }
}
