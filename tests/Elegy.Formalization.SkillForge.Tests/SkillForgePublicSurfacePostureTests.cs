using System;
using Xunit;

namespace Elegy.Formalization.SkillForge.Tests;

#pragma warning disable CS0618

public sealed class SkillForgePublicSurfacePostureTests
{
    [Fact]
    public void Compatibility_shell_types_remain_public_for_transition()
    {
        Assert.True(typeof(SkillForgeService).IsPublic);
        Assert.True(typeof(SkillForgeOptions).IsPublic);
        Assert.True(typeof(SkillForgeRequest).IsPublic);
        Assert.True(typeof(SkillForgeResult).IsPublic);
        Assert.True(typeof(RegistrationMetadata).IsPublic);
    }

    [Fact]
    public void Compatibility_shell_types_are_marked_obsolete()
    {
        AssertCompatibilitySurface(typeof(SkillForgeService));
        AssertCompatibilitySurface(typeof(SkillForgeOptions));
        AssertCompatibilitySurface(typeof(SkillForgeRequest));
        AssertCompatibilitySurface(typeof(SkillForgeResult));
        AssertCompatibilitySurface(typeof(RegistrationMetadata));
    }

    private static void AssertCompatibilitySurface(Type type)
    {
        var attribute = (ObsoleteAttribute?)Attribute.GetCustomAttribute(type, typeof(ObsoleteAttribute));
        Assert.NotNull(attribute);
        Assert.Contains("Compatibility surface only", attribute!.Message, StringComparison.Ordinal);
    }
}

#pragma warning restore CS0618