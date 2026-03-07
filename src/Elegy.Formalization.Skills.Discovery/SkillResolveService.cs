namespace Elegy.Formalization.Skills.Discovery;

public enum SkillResolveError
{
    NotFound,
    PathTraversal,
    SymlinkDetected,
    OutsideVault,
    ReadError
}

public abstract record SkillResolveResult
{
    public sealed record Success(string Content, string ResolvedPath) : SkillResolveResult;
    public sealed record Failure(SkillResolveError Error, string Message) : SkillResolveResult;
}

public sealed class SkillResolveService
{
    public SkillResolveResult Resolve(SkillIndexEntry entry, string vaultRootPath)
    {
        if (string.IsNullOrWhiteSpace(entry.VaultRef))
        {
            return new SkillResolveResult.Failure(
                SkillResolveError.NotFound,
                $"No VaultRef for skill '{entry.Id}'.");
        }

        var vaultRef = entry.VaultRef;

        // Reject path traversal segments
        if (vaultRef.Contains("..", StringComparison.Ordinal))
        {
            return new SkillResolveResult.Failure(
                SkillResolveError.PathTraversal,
                $"VaultRef '{vaultRef}' contains path traversal segments.");
        }

        var fullVaultRoot = Path.GetFullPath(vaultRootPath);
        var candidatePath = Path.GetFullPath(Path.Combine(fullVaultRoot, vaultRef));

        // Confinement check: resolved path must be under vault root
        if (!candidatePath.StartsWith(fullVaultRoot, StringComparison.OrdinalIgnoreCase))
        {
            return new SkillResolveResult.Failure(
                SkillResolveError.OutsideVault,
                $"Resolved path escapes vault root.");
        }

        if (!File.Exists(candidatePath))
        {
            return new SkillResolveResult.Failure(
                SkillResolveError.NotFound,
                $"File not found: '{candidatePath}'.");
        }

        // Symlink detection
        try
        {
            var attrs = File.GetAttributes(candidatePath);
            if (attrs.HasFlag(FileAttributes.ReparsePoint))
            {
                return new SkillResolveResult.Failure(
                    SkillResolveError.SymlinkDetected,
                    $"Symlink detected at '{candidatePath}'.");
            }
        }
        catch (Exception ex)
        {
            return new SkillResolveResult.Failure(
                SkillResolveError.ReadError,
                $"Error checking file attributes: {ex.Message}");
        }

        try
        {
            var content = File.ReadAllText(candidatePath);
            return new SkillResolveResult.Success(content, candidatePath);
        }
        catch (Exception ex)
        {
            return new SkillResolveResult.Failure(
                SkillResolveError.ReadError,
                $"Error reading file: {ex.Message}");
        }
    }
}
