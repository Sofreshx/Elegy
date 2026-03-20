namespace Elegy.Formalization.Contracts;

public static class FormalizationArtifactNames
{
    public const string AgentCreateRequestSchema = "agent-create-request.schema.json";
    public const string AgentDefinitionSchema = "agent-definition.schema.json";
    public const string CanonicalWorkflowSchema = "canonical-workflow.schema.json";
    public const string CanonicalWorkflowGraphSchema = "canonical-workflow-graph.schema.json";
    public const string CompatibilityManifest = "compatibility-manifest.json";
    public const string CompatibilityMatrix = "compatibility-matrix.json";
    public const string DynamicSkillActivationSchema = "dynamic-skill-activation.schema.json";
    public const string McpAnalysisResultSchema = "mcp-analysis-result.schema.json";
    public const string McpServerDescriptorSchema = "mcp-server-descriptor.schema.json";
    public const string McpToolDefinitionSchema = "mcp-tool-definition.schema.json";
    public const string MonitoringEventSchema = "monitoring-event.schema.json";
    public const string SkillDefinitionSchema = "skill-definition.schema.json";
    public const string SkillDiscoveryIndexSchema = "skill-discovery-index.schema.json";
    public const string SkillForgeRequestSchema = "skill-forge-request.schema.json";
    public const string AgentCreateRequestMinimalFixture = "fixtures/agent-create-request.minimal.json";
    public const string AgentDefinitionMinimalFixture = "fixtures/agent-definition.minimal.json";
    public const string CanonicalWorkflowGraphMinimalFixture = "fixtures/canonical-workflow-graph.minimal.json";
    public const string CanonicalWorkflowMinimalFixture = "fixtures/canonical-workflow.minimal.json";
    public const string DynamicSkillActivationMinimalFixture = "fixtures/dynamic-skill-activation.minimal.json";
    public const string McpAnalysisResultMinimalFixture = "fixtures/mcp-analysis-result.minimal.json";
    public const string McpAnalysisResultParityFixture = "fixtures/mcp-analysis-result.parity.json";
    public const string McpParityExpectedFixture = "fixtures/mcp-parity-expected.json";
    public const string McpServerDescriptorMinimalFixture = "fixtures/mcp-server-descriptor.minimal.json";
    public const string McpServerDescriptorParityFixture = "fixtures/mcp-server-descriptor.parity.json";
    public const string McpToolDefinitionMinimalFixture = "fixtures/mcp-tool-definition.minimal.json";
    public const string MonitoringEventMinimalFixture = "fixtures/monitoring-event.minimal.json";
    public const string SkillDefinitionMinimalFixture = "fixtures/skill-definition.minimal.json";
    public const string SkillDiscoveryIndexMinimalFixture = "fixtures/skill-discovery-index.minimal.json";
    public const string SkillForgeRequestMinimalFixture = "fixtures/skill-forge-request.minimal.json";

    public static IReadOnlyList<string> All { get; } =
    [
        AgentCreateRequestSchema,
        AgentDefinitionSchema,
        CanonicalWorkflowGraphSchema,
        CanonicalWorkflowSchema,
        CompatibilityManifest,
        CompatibilityMatrix,
        DynamicSkillActivationSchema,
        McpAnalysisResultSchema,
        McpServerDescriptorSchema,
        McpToolDefinitionSchema,
        MonitoringEventSchema,
        SkillDefinitionSchema,
        SkillDiscoveryIndexSchema,
        SkillForgeRequestSchema,
        AgentCreateRequestMinimalFixture,
        AgentDefinitionMinimalFixture,
        CanonicalWorkflowGraphMinimalFixture,
        CanonicalWorkflowMinimalFixture,
        DynamicSkillActivationMinimalFixture,
        McpAnalysisResultMinimalFixture,
        McpAnalysisResultParityFixture,
        McpParityExpectedFixture,
        McpServerDescriptorMinimalFixture,
        McpServerDescriptorParityFixture,
        McpToolDefinitionMinimalFixture,
        MonitoringEventMinimalFixture,
        SkillDefinitionMinimalFixture,
        SkillDiscoveryIndexMinimalFixture,
        SkillForgeRequestMinimalFixture
    ];
}
