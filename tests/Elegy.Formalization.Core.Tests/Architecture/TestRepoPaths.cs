namespace Elegy.Formalization.Core.Tests.Architecture;

internal static class TestRepoPaths
{
    public static string RepoRoot { get; } = FindRepoRoot();

    public static string SourceRoot => Path.Combine(RepoRoot, "src");

    private static string FindRepoRoot()
    {
        var current = new DirectoryInfo(AppContext.BaseDirectory);

        while (current is not null)
        {
            if (File.Exists(Path.Combine(current.FullName, "Elegy.sln")))
            {
                return current.FullName;
            }

            current = current.Parent;
        }

        throw new DirectoryNotFoundException("Could not locate the Elegy repo root from the test output directory.");
    }
}