using System;
using Xunit;

namespace Elegy.Formalization.DynamicSkills.Tests;

#pragma warning disable CS0618

public sealed class DynamicSkillsPublicSurfacePostureTests
{
    [Fact]
    public void Compatibility_shell_types_remain_public_for_transition()
    {
        Assert.True(typeof(DynamicSkillEngine).IsPublic);
        Assert.True(typeof(DynamicSkillEngineOptions).IsPublic);
        Assert.True(typeof(DynamicSkillCreateRequest).IsPublic);
        Assert.True(typeof(DynamicSkillCreateResult).IsPublic);
    }

    [Fact]
    public void Compatibility_shell_types_are_marked_obsolete()
    {
        AssertCompatibilitySurface(typeof(DynamicSkillEngine));
        AssertCompatibilitySurface(typeof(DynamicSkillEngineOptions));
        AssertCompatibilitySurface(typeof(DynamicSkillCreateRequest));
        AssertCompatibilitySurface(typeof(DynamicSkillCreateResult));
    }

    private static void AssertCompatibilitySurface(Type type)
    {
        var attribute = (ObsoleteAttribute?)Attribute.GetCustomAttribute(type, typeof(ObsoleteAttribute));
        Assert.NotNull(attribute);
        Assert.Contains("Compatibility surface only", attribute!.Message, StringComparison.Ordinal);
    }
}

#pragma warning restore CS0618