using System.Text.Json;
using System.Xml.Linq;
using Xunit;

namespace Elegy.Formalization.Core.Tests.Architecture;

public sealed class ContractsArtifactGovernanceTests
{
    [Fact]
    public void Compatibility_Manifest_Package_Version_Matches_Directory_Build_Props()
    {
        var packageVersion = GetPackageVersion();
        using var manifest = LoadJson(Path.Combine(TestRepoPaths.SourceRoot, "Elegy.Formalization.Contracts", "Resources", "compatibility-manifest.json"));

        var manifestVersion = manifest.RootElement
            .GetProperty("package")
            .GetProperty("version")
            .GetString();

        Assert.Equal(packageVersion, manifestVersion);
    }

    [Fact]
    public void Compatibility_Manifest_Schema_Version_Matches_Schema_Version_File()
    {
        using var schemaVersionDocument = LoadJson(Path.Combine(TestRepoPaths.RepoRoot, "schemas", "schema-version.json"));
        using var manifest = LoadJson(Path.Combine(TestRepoPaths.SourceRoot, "Elegy.Formalization.Contracts", "Resources", "compatibility-manifest.json"));

        var schemaVersion = schemaVersionDocument.RootElement.GetProperty("schemaVersion").GetString();
        var manifestSchemaVersion = manifest.RootElement
            .GetProperty("schemas")
            .EnumerateArray()
            .Single(static element => element.GetProperty("name").GetString() == "canonical-workflow")
            .GetProperty("schemaVersion")
            .GetString();

        Assert.Equal(schemaVersion, manifestSchemaVersion);
    }

    [Fact]
    public void Compatibility_Manifest_References_Existing_Schema_And_Fixture_Files()
    {
        using var manifest = LoadJson(Path.Combine(TestRepoPaths.SourceRoot, "Elegy.Formalization.Contracts", "Resources", "compatibility-manifest.json"));
        var resourcesRoot = Path.Combine(TestRepoPaths.SourceRoot, "Elegy.Formalization.Contracts", "Resources");

        foreach (var schemaEntry in manifest.RootElement.GetProperty("schemas").EnumerateArray())
        {
            var schemaFile = schemaEntry.GetProperty("file").GetString();
            Assert.False(string.IsNullOrWhiteSpace(schemaFile));
            Assert.True(File.Exists(Path.Combine(resourcesRoot, schemaFile!)), $"Schema file '{schemaFile}' was referenced in the compatibility manifest but was not found.");

            foreach (var fixtureEntry in schemaEntry.GetProperty("fixtures").EnumerateArray())
            {
                var fixture = fixtureEntry.GetString();
                Assert.False(string.IsNullOrWhiteSpace(fixture));
                Assert.True(File.Exists(Path.Combine(resourcesRoot, fixture!)), $"Fixture '{fixture}' was referenced in the compatibility manifest but was not found.");
            }
        }
    }

    [Fact]
    public void Compatibility_Manifest_Advertises_Canonical_Workflow_Graph_Artifacts()
    {
        using var manifest = LoadJson(Path.Combine(TestRepoPaths.SourceRoot, "Elegy.Formalization.Contracts", "Resources", "compatibility-manifest.json"));

        var schemaEntry = manifest.RootElement
            .GetProperty("schemas")
            .EnumerateArray()
            .Single(static element => element.GetProperty("name").GetString() == "canonical-workflow-graph");

        Assert.Equal("canonical-workflow-graph.schema.json", schemaEntry.GetProperty("file").GetString());
        Assert.Contains(
            "fixtures/canonical-workflow-graph.minimal.json",
            schemaEntry.GetProperty("fixtures").EnumerateArray().Select(static fixture => fixture.GetString()));
    }

    [Fact]
    public void Compatibility_Matrix_Defines_At_Least_One_Entry()
    {
        using var matrix = LoadJson(Path.Combine(TestRepoPaths.SourceRoot, "Elegy.Formalization.Contracts", "Resources", "compatibility-matrix.json"));

        Assert.False(string.IsNullOrWhiteSpace(matrix.RootElement.GetProperty("matrixVersion").GetString()));
        Assert.NotEmpty(matrix.RootElement.GetProperty("entries").EnumerateArray());
    }

    private static JsonDocument LoadJson(string path)
    {
        return JsonDocument.Parse(File.ReadAllText(path));
    }

    private static string? GetPackageVersion()
    {
        var document = XDocument.Load(Path.Combine(TestRepoPaths.RepoRoot, "Directory.Build.props"));
        return document.Root?
            .Element("PropertyGroup")?
            .Element("VersionPrefix")?
            .Value;
    }
}
