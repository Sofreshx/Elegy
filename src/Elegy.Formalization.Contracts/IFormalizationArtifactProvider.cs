namespace Elegy.Formalization.Contracts;

public interface IFormalizationArtifactProvider
{
    IReadOnlyList<string> GetArtifactNames();

    bool Exists(string artifactName);

    Stream OpenRead(string artifactName);

    string ReadText(string artifactName);
}
