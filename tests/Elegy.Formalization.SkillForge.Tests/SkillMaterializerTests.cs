using Elegy.Formalization.DynamicSkills;
using Elegy.Formalization.Skills;
using Elegy.Formalization.SkillForge;
using Xunit;

namespace Elegy.Formalization.SkillForge.Tests;

public sealed class SkillMaterializerTests : IDisposable
{
    private readonly string _tempRoot;

    public SkillMaterializerTests()
    {
        _tempRoot = Path.Combine(Path.GetTempPath(), "elegy-test-" + Guid.NewGuid().ToString("N"));
        Directory.CreateDirectory(_tempRoot);
    }

    public void Dispose()
    {
        if (Directory.Exists(_tempRoot))
            Directory.Delete(_tempRoot, recursive: true);
    }

    private static SkillForgeResult CreateSuccessResult() => new()
    {
        Success = true,
        CreatedSkill = new SkillDefinition
        {
            Id = "my-skill",
            Name = "My Skill",
            Description = "A test skill for testing",
            Metadata = new SkillMetadata
            {
                Summary = "A test skill for testing",
            },
            Triggers =
            [
                new SkillTrigger { Pattern = "test trigger", Description = "fires on test" },
                new SkillTrigger { Pattern = "unit test", Description = "fires on unit test" }
            ],
            Discovery = new SkillDiscoveryMetadata
            {
                Keywords = ["testing", "unit"],
                CapabilityHints = ["lookup"],
            },
            Input = new SkillInputContract
            {
                Parameters = [new SkillParameter { Name = "query", Type = "string", Description = "Search query" }]
            },
            Governance = new SkillGovernanceMetadata
            {
                RiskLevel = SkillRiskLevel.Medium,
                AllowedContexts = ["workspace"],
            },
            Constraints =
            [
                new SkillConstraint { ConstraintId = "c1", Description = "must be valid", Required = true },
                new SkillConstraint { ConstraintId = "c2", Description = "optional check", Required = false }
            ]
        },
        RegistrationMetadata = new RegistrationMetadata
        {
            ManifestEntry = "my-skill",
            SkillId = "my-skill",
            DiscoveryKeywords = ["testing", "unit"],
            MaterializationKind = SkillMaterializationKind.Dynamic,
            SourceKind = SkillSourceKind.Imported,
        }
    };

    private SkillMaterializer CreateMaterializer() =>
        new(new SkillMaterializerOptions { VaultRootPath = _tempRoot });

    [Fact]
    public void Materialize_ValidResult_CreatesSkillFile()
    {
        var materializer = CreateMaterializer();
        var result = materializer.Materialize(CreateSuccessResult());

        Assert.True(result.Success);
        Assert.NotNull(result.WrittenPath);
        Assert.True(File.Exists(result.WrittenPath));

        var expectedPath = Path.Combine(_tempRoot, "my-skill", "SKILL.md");
        Assert.Equal(expectedPath, result.WrittenPath);
    }

    [Fact]
    public void Materialize_ValidResult_GeneratesYamlFrontmatter()
    {
        var materializer = CreateMaterializer();
        materializer.Materialize(CreateSuccessResult());

        var content = File.ReadAllText(Path.Combine(_tempRoot, "my-skill", "SKILL.md"));

        Assert.StartsWith("---", content);
        Assert.Contains("name: My Skill", content);
        Assert.Contains("description: A test skill for testing", content);
        Assert.Contains("triggersOn: test trigger, unit test", content);
        Assert.Contains("keywords: testing, unit", content);
        Assert.Contains("capabilityHints: lookup", content);
        Assert.Contains("lifecycleState: draft", content);
    }

    [Fact]
    public void Materialize_ValidResult_GeneratesBodySections()
    {
        var materializer = CreateMaterializer();
        materializer.Materialize(CreateSuccessResult());

        var content = File.ReadAllText(Path.Combine(_tempRoot, "my-skill", "SKILL.md"));

        Assert.Contains("# My Skill", content);
        Assert.Contains("## Purpose", content);
        Assert.Contains("A test skill for testing", content);
        Assert.Contains("## When to Use", content);
        Assert.Contains("- **test trigger**: fires on test", content);
        Assert.Contains("- **unit test**: fires on unit test", content);
        Assert.Contains("## Inputs", content);
        Assert.Contains("`query` (string): Search query", content);
        Assert.Contains("## Governance", content);
        Assert.Contains("Risk level: Medium", content);
        Assert.Contains("Allowed contexts: workspace", content);
        Assert.Contains("## Constraints", content);
        Assert.Contains("**(required)** `c1`: must be valid", content);
        Assert.Contains("`c2`: optional check", content);
    }

    [Fact]
    public void Materialize_FailedForgeResult_ReturnsFalse()
    {
        var materializer = CreateMaterializer();
        var forgeResult = new SkillForgeResult
        {
            Success = false,
            ErrorMessage = "forge failed"
        };

        var result = materializer.Materialize(forgeResult);

        Assert.False(result.Success);
        Assert.Contains("failed forge result", result.ErrorMessage);
    }

    [Fact]
    public void Materialize_ExistingSkillFile_FailsClosed()
    {
        var materializer = CreateMaterializer();

        // First write succeeds
        var first = materializer.Materialize(CreateSuccessResult());
        Assert.True(first.Success);

        // Second write to same skill fails
        var second = materializer.Materialize(CreateSuccessResult());
        Assert.False(second.Success);
        Assert.Contains("already exists", second.ErrorMessage);
    }

    [Theory]
    [InlineData("../escape")]
    [InlineData("..\\escape")]
    [InlineData("some/../../escape")]
    [InlineData("some\\..\\escape")]
    public void Materialize_PathTraversal_ReturnsFalse(string maliciousName)
    {
        var materializer = CreateMaterializer();
        var forgeResult = CreateSuccessResult() with
        {
            CreatedSkill = CreateSuccessResult().CreatedSkill! with { Id = maliciousName }
        };

        var result = materializer.Materialize(forgeResult);

        Assert.False(result.Success);
        Assert.NotNull(result.ErrorMessage);
    }

    [Fact]
    public void Materialize_NullCreatedSkill_ReturnsFalse()
    {
        var materializer = CreateMaterializer();
        var forgeResult = new SkillForgeResult
        {
            Success = true,
            CreatedSkill = null
        };

        var result = materializer.Materialize(forgeResult);

        Assert.False(result.Success);
    }

    [Fact]
    public void Materialize_EmptySkillName_ReturnsFalse()
    {
        var materializer = CreateMaterializer();
        var forgeResult = CreateSuccessResult() with
        {
            CreatedSkill = CreateSuccessResult().CreatedSkill! with { Id = "" }
        };

        var result = materializer.Materialize(forgeResult);

        Assert.False(result.Success);
        Assert.Contains("identifier", result.ErrorMessage, StringComparison.OrdinalIgnoreCase);
    }

    [Fact]
    public void Materialize_Conformance_Path_Preserves_Canonical_Semantics()
    {
        var service = new SkillForgeService(
            new DynamicSkillEngine(new DynamicSkillEngineOptions { IsEnabled = true }),
            new SkillForgeOptions());
        var materializer = CreateMaterializer();

        var forgeResult = service.Forge(new SkillForgeRequest
        {
            SkillId = "canonical-skill",
            Name = "Canonical Skill",
            Description = "Canonical downstream skill",
            Triggers = [new SkillTrigger { Pattern = "canonical" }],
            Constraints = [new SkillConstraint { ConstraintId = "approved", Required = true }],
            DiscoveryKeywords = ["testing"],
            Discovery = new SkillDiscoveryMetadata { CapabilityHints = ["lookup"] },
            Input = new SkillInputContract { Parameters = [new SkillParameter { Name = "query", Type = "string" }] },
            Governance = new SkillGovernanceMetadata { RiskLevel = SkillRiskLevel.Medium, AllowedContexts = ["workspace"] },
            Origin = new SkillOrigin { SourceKind = SkillSourceKind.Imported, SourceRef = "import://catalog/canonical-skill" },
        });

        Assert.True(forgeResult.Success);
        Assert.Equal("canonical-skill", forgeResult.CreatedSkill!.EffectiveId);
        Assert.Equal(SkillMaterializationKind.Dynamic, forgeResult.CreatedSkill.Origin.MaterializationKind);
        Assert.Equal(SkillSourceKind.Imported, forgeResult.CreatedSkill.Origin.SourceKind);
        Assert.Equal("query", Assert.Single(forgeResult.CreatedSkill.Input.Parameters).Name);

        var materializeResult = materializer.Materialize(forgeResult);
        Assert.True(materializeResult.Success);

        var content = File.ReadAllText(materializeResult.WrittenPath!);
        Assert.Contains("name: Canonical Skill", content);
        Assert.Contains("keywords: testing", content);
        Assert.Contains("capabilityHints: lookup", content);
        Assert.Contains("Risk level: Medium", content);
    }
}
