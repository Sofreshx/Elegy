use elegy_plugin_sdk::ElegyCapabilityInvocationV2;
use serde_json::json;

#[test]
fn cli_invocations_round_trip_input_and_output_schemas() {
    let invocation = ElegyCapabilityInvocationV2 {
        executable: "./bin/example-tool".to_string(),
        command: vec!["--json".to_string()],
        required_args: Vec::new(),
        optional_args: Vec::new(),
        input_schema: Some(json!({
            "type": "object",
            "properties": {"value": {"type": "string"}},
            "required": ["value"]
        })),
        output_schema: Some(json!({
            "type": "object",
            "properties": {"echo": {"type": "string"}},
            "required": ["echo"]
        })),
    };

    let encoded = serde_json::to_value(&invocation).expect("serialize invocation");
    assert_eq!(encoded["inputSchema"]["type"], "object");
    assert_eq!(encoded["outputSchema"]["type"], "object");
    assert_eq!(
        serde_json::from_value::<ElegyCapabilityInvocationV2>(encoded)
            .expect("deserialize invocation"),
        invocation
    );
}
