using Elegy.Formalization.Contracts;
using Xunit;

namespace Elegy.Formalization.Core.Tests.Architecture;

public sealed class EmbeddedFormalizationArtifactProviderTests
{
    private static readonly string ResourcesRoot = Path.Combine(
        TestRepoPaths.SourceRoot,
        "Elegy.Formalization.Contracts",
        "Resources");

    [Fact]
    public void GetArtifactNames_Matches_Tracked_Resource_Files()
    {
        var provider = EmbeddedFormalizationArtifactProvider.Default;

        var expectedArtifactNames = Directory
            .GetFiles(ResourcesRoot, "*.json", SearchOption.AllDirectories)
            .Select(static path => Path.GetRelativePath(ResourcesRoot, path).Replace('\\', '/'))
            .OrderBy(static path => path, StringComparer.Ordinal)
            .ToArray();

        var actualArtifactNames = provider.GetArtifactNames()
            .OrderBy(static path => path, StringComparer.Ordinal)
            .ToArray();

        Assert.Equal(expectedArtifactNames, actualArtifactNames);
    }

    [Fact]
    public void ReadText_Loads_Embedded_Manifest_Content()
    {
        var provider = EmbeddedFormalizationArtifactProvider.Default;
        var expected = File.ReadAllText(Path.Combine(ResourcesRoot, FormalizationArtifactNames.CompatibilityManifest));

        var actual = provider.ReadText(FormalizationArtifactNames.CompatibilityManifest);

        Assert.Equal(expected, actual);
    }

    [Fact]
    public void Exists_And_OpenRead_Work_For_Fixture_Artifacts()
    {
        var provider = EmbeddedFormalizationArtifactProvider.Default;

        Assert.True(provider.Exists(FormalizationArtifactNames.McpServerDescriptorMinimalFixture));

        using var stream = provider.OpenRead(FormalizationArtifactNames.McpServerDescriptorMinimalFixture);
        Assert.True(stream.Length > 0);
    }
}
