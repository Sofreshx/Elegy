using Elegy.Formalization.Skills;
using Xunit;

namespace Elegy.Formalization.Skills.Tests;

public sealed class SkillDefinitionTests
{
    [Fact]
    public void Default_Id_Is_Empty()
    {
        var def = new SkillDefinition();
        Assert.Equal(string.Empty, def.Id);
    }

    [Fact]
    public void Default_LifecycleState_Is_Draft()
    {
        var def = new SkillDefinition();
        Assert.Equal(SkillLifecycleState.Draft, def.LifecycleState);
    }

    [Fact]
    public void Default_Triggers_Is_Empty()
    {
        var def = new SkillDefinition();
        Assert.Empty(def.Triggers);
    }

    [Fact]
    public void Default_Constraints_Is_Empty()
    {
        var def = new SkillDefinition();
        Assert.Empty(def.Constraints);
    }

    [Fact]
    public void Default_Richer_Contract_Blocks_Are_Present()
    {
        var def = new SkillDefinition();

        Assert.NotNull(def.Identity);
        Assert.NotNull(def.Metadata);
        Assert.NotNull(def.Input);
        Assert.NotNull(def.Output);
        Assert.NotNull(def.Execution);
        Assert.NotNull(def.Governance);
        Assert.NotNull(def.Discovery);
        Assert.NotNull(def.Origin);
    }

    [Fact]
    public void Effective_Id_And_Name_Fall_Back_To_Legacy_Fields()
    {
        var def = new SkillDefinition
        {
            Id = "skill.legacy",
            Name = "Legacy skill",
        };

        Assert.Equal("skill.legacy", def.EffectiveId);
        Assert.Equal("Legacy skill", def.EffectiveName);
    }

    [Fact]
    public void Effective_Id_And_Name_Prefer_Identity_Block_When_Present()
    {
        var def = new SkillDefinition
        {
            Id = "skill.legacy",
            Name = "Legacy skill",
            Identity = new SkillIdentity
            {
                DefinitionId = "skill.authoritative",
                DisplayName = "Authoritative skill",
            },
        };

        Assert.Equal("skill.authoritative", def.EffectiveId);
        Assert.Equal("Authoritative skill", def.EffectiveName);
    }
}
