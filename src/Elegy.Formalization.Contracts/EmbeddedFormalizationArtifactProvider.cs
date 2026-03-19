using System.Reflection;
using System.Text;

namespace Elegy.Formalization.Contracts;

public sealed class EmbeddedFormalizationArtifactProvider : IFormalizationArtifactProvider
{
    public static EmbeddedFormalizationArtifactProvider Default { get; } = new();

    private static readonly string ResourcePrefix = $"{typeof(EmbeddedFormalizationArtifactProvider).Assembly.GetName().Name}.Resources.";
    private readonly Assembly _assembly;

    public EmbeddedFormalizationArtifactProvider()
        : this(typeof(EmbeddedFormalizationArtifactProvider).Assembly)
    {
    }

    private EmbeddedFormalizationArtifactProvider(Assembly assembly)
    {
        _assembly = assembly;
    }

    public IReadOnlyList<string> GetArtifactNames()
    {
        return FormalizationArtifactNames.All;
    }

    public bool Exists(string artifactName)
    {
        using var stream = OpenReadCore(artifactName);
        return stream is not null;
    }

    public Stream OpenRead(string artifactName)
    {
        return OpenReadCore(artifactName)
            ?? throw new FileNotFoundException($"Embedded formalization artifact '{NormalizeArtifactName(artifactName)}' was not found.");
    }

    public string ReadText(string artifactName)
    {
        using var stream = OpenRead(artifactName);
        using var reader = new StreamReader(stream, Encoding.UTF8, detectEncodingFromByteOrderMarks: true);
        return reader.ReadToEnd();
    }

    private Stream? OpenReadCore(string artifactName)
    {
        var normalizedArtifactName = NormalizeArtifactName(artifactName);
        var resourceName = $"{ResourcePrefix}{normalizedArtifactName.Replace('/', '.')}";
        return _assembly.GetManifestResourceStream(resourceName);
    }

    private static string NormalizeArtifactName(string artifactName)
    {
        ArgumentException.ThrowIfNullOrWhiteSpace(artifactName);
        return artifactName.Trim().Replace('\\', '/').TrimStart('/');
    }
}
