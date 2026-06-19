
fn execute_diagram_create_command(
    diagram_type: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let diagram = CanonicalDiagram {
        diagram_type,
        version: 1,
        nodes: Vec::new(),
        edges: Vec::new(),
        groups: Vec::new(),
    };
    
    match format {
        OutputFormat::Text => println!("Created empty diagram of type: {}", diagram.diagram_type),
        OutputFormat::Json => print_json(&build_envelope(
            ["diagram", "create"],
            "ok",
            Summary::default(),
            &diagram,
            Vec::new(),
        ))?,
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_diagram_patch_command(
    input: PathBuf,
    add_node: Option<String>,
    add_edge: Option<String>,
    remove_node: Option<String>,
    remove_edge: Option<String>,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let content = match std::fs::read_to_string(&input) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Failed to read diagram file: {}", e);
            return Ok(exit_invalid());
        }
    };
    let mut diagram: CanonicalDiagram = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Failed to parse JSON diagram: {}", e);
            return Ok(exit_invalid());
        }
    };

    let mut patch = DiagramPatch::default();
    
    if let Some(id) = remove_node { patch.remove_node_ids.push(id) }
    if let Some(id) = remove_edge { patch.remove_edge_ids.push(id) }
    
    if let Some(n) = add_node {
        let parts: Vec<&str> = n.split(',').collect();
        if parts.len() >= 2 {
            patch.add_nodes.push(DiagramNode {
                id: parts[0].to_string(),
                label: parts[1].to_string(),
                concept_type: parts.get(2).map(|s| s.to_string()),
                properties: Default::default(),
            });
        }
    }
    
    if let Some(e) = add_edge {
        let parts: Vec<&str> = e.split(',').collect();
        if parts.len() >= 3 {
            patch.add_edges.push(DiagramEdge {
                id: parts[0].to_string(),
                source_id: parts[1].to_string(),
                target_id: parts[2].to_string(),
                label: parts.get(3).map(|s| s.to_string()),
                relationship_type: None,
                properties: Default::default(),
            });
        }
    }

    diagram.apply_patch(patch);
    
    if let Err(e) = diagram.validate() {
        eprintln!("Invalid patch resulted in invalid diagram: {}", e);
        return Ok(exit_invalid());
    }

    match format {
        OutputFormat::Text => println!("Diagram patched successfully."),
        OutputFormat::Json => print_json(&build_envelope(
            ["diagram", "patch"],
            "ok",
            Summary::default(),
            &diagram,
            Vec::new(),
        ))?,
    }

    Ok(ExitCode::SUCCESS)
}

fn execute_diagram_narrate_command(
    input: PathBuf,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let content = match std::fs::read_to_string(&input) {
        Ok(c) => c,
        Err(_) => return Ok(exit_invalid()),
    };
    let diagram: CanonicalDiagram = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(_) => return Ok(exit_invalid()),
    };
    
    let narrative = diagram.narrate_diagram();
    
    match format {
        OutputFormat::Text => println!("{}", narrative),
        OutputFormat::Json => print_json(&build_envelope(
            ["diagram", "narrate"],
            "ok",
            Summary::default(),
            json!({ "narrative": narrative }),
            Vec::new(),
        ))?,
    }
    Ok(ExitCode::SUCCESS)
}

fn execute_diagram_render_command(
    input: PathBuf,
    render_format: String,
    format: OutputFormat,
) -> Result<ExitCode, serde_json::Error> {
    let content = match std::fs::read_to_string(&input) {
        Ok(c) => c,
        Err(_) => return Ok(exit_invalid()),
    };
    let diagram: CanonicalDiagram = match serde_json::from_str(&content) {
        Ok(d) => d,
        Err(_) => return Ok(exit_invalid()),
    };
    
    let rendered = if render_format == "mermaid" {
        diagram.render_mermaid()
    } else {
        serde_json::to_string_pretty(&diagram).unwrap()
    };
    
    match format {
        OutputFormat::Text => println!("{}", rendered),
        OutputFormat::Json => print_json(&build_envelope(
            ["diagram", "render"],
            "ok",
            Summary::default(),
            json!({ "rendered": rendered, "format": render_format }),
            Vec::new(),
        ))?,
    }
    Ok(ExitCode::SUCCESS)
}
