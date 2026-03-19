using System.Xml.Linq;
using Xunit;

namespace Elegy.Formalization.Core.Tests.Architecture;

public sealed class PackageBoundaryPolicyTests
{
    private static readonly IReadOnlyDictionary<string, string[]> AllowedReferences = new Dictionary<string, string[]>(StringComparer.Ordinal)
    {
        ["Elegy.Formalization.Core"] = [],
        ["Elegy.Formalization.Contracts"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Serialization"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Validation"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Governance"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Projections.Mermaid"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Skills"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Skills.Discovery"] = ["Elegy.Formalization.Core", "Elegy.Formalization.Skills"],
        ["Elegy.Formalization.DynamicSkills"] = ["Elegy.Formalization.Core", "Elegy.Formalization.Skills", "Elegy.Formalization.Monitoring"],
        ["Elegy.Formalization.Monitoring"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.Mcp"] = ["Elegy.Formalization.Skills"],
        ["Elegy.Formalization.SkillForge"] = ["Elegy.Formalization.Core", "Elegy.Formalization.Skills", "Elegy.Formalization.DynamicSkills", "Elegy.Formalization.Governance"],
        ["Elegy.Formalization.Agents"] = ["Elegy.Formalization.Core"],
        ["Elegy.Formalization.AgentFactory"] = ["Elegy.Formalization.Core", "Elegy.Formalization.Agents", "Elegy.Formalization.Governance"],
    };

    private static readonly string[] SubstrateProjects =
    [
        "Elegy.Formalization.Core",
        "Elegy.Formalization.Contracts",
        "Elegy.Formalization.Serialization",
        "Elegy.Formalization.Validation",
        "Elegy.Formalization.Governance",
        "Elegy.Formalization.Projections.Mermaid",
    ];

    [Fact]
    public void Every_Source_Project_Has_A_Boundary_Policy()
    {
        var sourceProjects = GetSourceProjects();

        Assert.Equal(
            sourceProjects.Keys.OrderBy(static value => value),
            AllowedReferences.Keys.OrderBy(static value => value));
    }

    [Fact]
    public void Source_Project_References_Stay_Within_Declared_Policy()
    {
        var sourceProjects = GetSourceProjects();
        var violations = new List<string>();

        foreach (var (projectName, projectPath) in sourceProjects.OrderBy(static pair => pair.Key, StringComparer.Ordinal))
        {
            var allowedReferences = AllowedReferences[projectName];
            var actualReferences = GetProjectReferences(projectPath);

            foreach (var actualReference in actualReferences)
            {
                if (!sourceProjects.ContainsKey(actualReference))
                {
                    violations.Add($"{projectName} references unknown source project '{actualReference}'.");
                    continue;
                }

                if (!allowedReferences.Contains(actualReference, StringComparer.Ordinal))
                {
                    violations.Add($"{projectName} references {actualReference}, but the allowed policy is [{string.Join(", ", allowedReferences)}].");
                }
            }
        }

        Assert.True(violations.Count == 0, string.Join(Environment.NewLine, violations));
    }

    [Fact]
    public void Substrate_Projects_Do_Not_Depend_On_Higher_Level_Families()
    {
        var sourceProjects = GetSourceProjects();
        var violations = new List<string>();

        foreach (var projectName in SubstrateProjects)
        {
            var references = GetProjectReferences(sourceProjects[projectName]);
            foreach (var reference in references)
            {
                if (!SubstrateProjects.Contains(reference, StringComparer.Ordinal))
                {
                    violations.Add($"Substrate project {projectName} must not depend on higher-level family {reference}.");
                }
            }
        }

        Assert.True(violations.Count == 0, string.Join(Environment.NewLine, violations));
    }

    private static Dictionary<string, string> GetSourceProjects()
    {
        return Directory
            .GetFiles(TestRepoPaths.SourceRoot, "*.csproj", SearchOption.AllDirectories)
            .ToDictionary(
                static path => Path.GetFileNameWithoutExtension(path),
                static path => path,
                StringComparer.Ordinal);
    }

    private static IReadOnlyList<string> GetProjectReferences(string projectPath)
    {
        var projectDirectory = Path.GetDirectoryName(projectPath)
            ?? throw new DirectoryNotFoundException($"Project path '{projectPath}' did not have a parent directory.");

        var document = XDocument.Load(projectPath);
        return document
            .Descendants("ProjectReference")
            .Select(static element => (string?)element.Attribute("Include"))
            .Where(static include => !string.IsNullOrWhiteSpace(include))
            .Select(include => Path.GetFullPath(Path.Combine(projectDirectory, include!)))
            .Select(Path.GetFileNameWithoutExtension)
            .Where(static value => !string.IsNullOrWhiteSpace(value))
            .Select(static value => value!)
            .OrderBy(static value => value, StringComparer.Ordinal)
            .ToArray();
    }
}