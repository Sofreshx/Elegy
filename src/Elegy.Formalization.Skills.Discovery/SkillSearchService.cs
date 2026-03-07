namespace Elegy.Formalization.Skills.Discovery;

public sealed record SkillSearchResult
{
    public required SkillIndexEntry Entry { get; init; }
    public int Score { get; init; }
    public string MatchReason { get; init; } = string.Empty;
}

public sealed class SkillSearchService
{
    public IReadOnlyList<SkillSearchResult> Search(SkillDiscoveryIndex index, string query)
    {
        if (string.IsNullOrWhiteSpace(query))
        {
            return index.Entries
                .Select(e => new SkillSearchResult { Entry = e, Score = 0, MatchReason = "all" })
                .ToList();
        }

        var results = new List<SkillSearchResult>();
        var q = query.Trim();

        foreach (var entry in index.Entries)
        {
            var (score, reason) = ScoreEntry(entry, q);
            if (score > 0)
            {
                results.Add(new SkillSearchResult { Entry = entry, Score = score, MatchReason = reason });
            }
        }

        return results.OrderByDescending(r => r.Score).ToList();
    }

    public IReadOnlyList<SkillSearchResult> SearchByStack(SkillDiscoveryIndex index, IReadOnlyList<string> stackTokens)
    {
        var seen = new HashSet<string>(StringComparer.OrdinalIgnoreCase);
        var results = new List<SkillSearchResult>();

        foreach (var token in stackTokens)
        {
            foreach (var result in Search(index, token))
            {
                if (seen.Add(result.Entry.Id))
                {
                    results.Add(result);
                }
            }
        }

        return results.OrderByDescending(r => r.Score).ToList();
    }

    private static (int Score, string Reason) ScoreEntry(SkillIndexEntry entry, string query)
    {
        var comparison = StringComparison.OrdinalIgnoreCase;

        if (entry.Name.Equals(query, comparison))
            return (100, "exact-name");

        if (entry.Name.Contains(query, comparison))
            return (50, "name-contains");

        foreach (var trigger in entry.Triggers)
        {
            if (trigger.Contains(query, comparison))
                return (30, "trigger-contains");
        }

        if (entry.Description is not null && entry.Description.Contains(query, comparison))
            return (10, "description-contains");

        return (0, string.Empty);
    }
}
