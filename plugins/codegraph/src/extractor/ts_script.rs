/// Embedded TypeScript extraction script.
///
/// This script is spawned via `node -e` and uses the TypeScript Compiler API
/// to walk a project's source files and emit a raw graph as JSON to stdout.
///
/// Expected: Node.js with TypeScript installed (npm install typescript) or
/// a local `node_modules` directory in the repo being analyzed.
pub const TS_EXTRACT_SCRIPT: &str = r#"
const ts = require('typescript');
const path = require('path');
const fs = require('fs');

const repoPath = process.argv[2];
if (!repoPath) {
    console.error('Usage: node extract-ts.js <repo-path>');
    process.exit(1);
}

// Find tsconfig.json
const configPath = ts.findConfigFile(repoPath, ts.sys.fileExists, 'tsconfig.json');
if (!configPath) {
    console.error('No tsconfig.json found in', repoPath);
    process.exit(1);
}

const configFile = ts.readConfigFile(configPath, ts.sys.readFile);
const parsedConfig = ts.parseJsonConfigFileContent(
    configFile.config, ts.sys, path.dirname(configPath)
);

const program = ts.createProgram(parsedConfig.fileNames, parsedConfig.options);
const checker = program.getTypeChecker();

const entities = [];
const edges = [];
const entityIds = new Map(); // qualifiedName|file|kind -> id

function entityId(qualifiedName, file, kind) {
    const key = `${qualifiedName}|${file}|${kind}`;
    if (!entityIds.has(key)) {
        // Simple counter-based ID for the raw script; Rust will compute SHA
        entityIds.set(key, `ts-${entityIds.size}`);
    }
    return entityIds.get(key);
}

function kindForNode(node) {
    switch (node.kind) {
        case ts.SyntaxKind.SourceFile: return 'file';
        case ts.SyntaxKind.ModuleDeclaration: return 'module';
        case ts.SyntaxKind.FunctionDeclaration: return 'function';
        case ts.SyntaxKind.MethodDeclaration: return 'method';
        case ts.SyntaxKind.ClassDeclaration: return 'class';
        case ts.SyntaxKind.InterfaceDeclaration: return 'interface';
        case ts.SyntaxKind.TypeAliasDeclaration: return 'type';
        case ts.SyntaxKind.EnumDeclaration: return 'enum';
        case ts.SyntaxKind.VariableDeclaration:
            if (node.parent && node.parent.flags & ts.NodeFlags.Const) return 'constant';
            return 'variable';
        default: return null;
    }
}

function getName(node) {
    if (node.name && ts.isIdentifier(node.name)) return node.name.text;
    if (node.name && ts.isStringLiteral(node.name)) return node.name.text;
    return '<anonymous>';
}

function getQualifiedName(node, sourceFile) {
    const parts = [];
    let current = node.parent;
    while (current && current !== sourceFile) {
        if (ts.isModuleDeclaration(current) || ts.isModuleBlock(current)) {
            if (current.name && ts.isIdentifier(current.name)) {
                parts.unshift(current.name.text);
            }
        }
        if (ts.isFunctionDeclaration(current) && current.name) {
            parts.unshift(current.name.text);
        }
        current = current.parent;
    }
    const fileName = path.relative(repoPath, sourceFile.fileName).replace(/\\/g, '/');
    parts.unshift(fileName.replace(/\.tsx?$/, ''));
    if (node.name && ts.isIdentifier(node.name)) {
        parts.push(node.name.text);
    }
    return parts.join('::');
}

function getSpan(node, sourceFile) {
    const start = sourceFile.getLineAndCharacterOfPosition(node.getStart(sourceFile));
    const end = sourceFile.getLineAndCharacterOfPosition(node.getEnd());
    return {
        start: [start.line + 1, start.character + 1],
        end: [end.line + 1, end.character + 1]
    };
}

function isTestFile(fileName) {
    return /\.(test|spec)\.tsx?$/.test(fileName) || fileName.includes('__tests__');
}

function getLayer(fileName) {
    if (isTestFile(fileName)) return 'test';
    return 'source';
}

function addEntity(kind, name, qualifiedName, file, span, sourceFile) {
    const layer = getLayer(file);
    const id = entityId(qualifiedName, file, kind);
    
    const existing = entities.find(e => e.id === id);
    if (existing) return existing;

    const entity = {
        id,
        kind,
        layer,
        name,
        qualifiedName,
        file,
        span: span ? { start: [span.start[0], span.start[1]], end: [span.end[0], span.end[1]] } : null,
        inputs: [],
        outputs: [],
        sideEffects: [],
        dependencies: [],
        tests: [],
        docs: [],
        rawExported: false,
        rawCalls: [],
        rawDocComment: null,
        sourceFile: sourceFile,
    };
    entities.push(entity);
    return entity;
}

// Process each source file
for (const sourceFile of program.getSourceFiles()) {
    if (sourceFile.isDeclarationFile) continue;
    
    const fileName = path.relative(repoPath, sourceFile.fileName).replace(/\\/g, '/');
    
    // Add file entity
    const fileSpan = getSpan(sourceFile, sourceFile);
    addEntity('file', fileName, fileName, fileName, fileSpan, sourceFile);

    function visit(node) {
        const kind = kindForNode(node);
        if (kind && kind !== 'file') {
            const name = getName(node);
            const qualifiedName = getQualifiedName(node, sourceFile);
            const span = getSpan(node, sourceFile);
            const entity = addEntity(kind, name, qualifiedName, fileName, span, sourceFile);

            // Check for export
            if (node.modifiers && node.modifiers.some(m => m.kind === ts.SyntaxKind.ExportKeyword)) {
                entity.rawExported = true;
            }
            if (node.parent && node.parent.kind === ts.SyntaxKind.SourceFile && 
                !node.modifiers) {
                // Default-exported or module-level declarations
            }

            // Check for JSDoc
            const jsDoc = node.jsDoc || [];
            if (jsDoc.length > 0) {
                entity.rawDocComment = jsDoc.map(d => d.comment || '').join('\n');
            }

            // Get return type for functions
            if (ts.isFunctionDeclaration(node) || ts.isMethodDeclaration(node)) {
                if (node.type) {
                    entity.outputs = [{ typeHint: node.type.getText(sourceFile) }];
                }
                node.parameters.forEach(p => {
                    entity.inputs.push({
                        name: p.name.getText(sourceFile),
                        typeHint: p.type ? p.type.getText(sourceFile) : null
                    });
                });
            }

            // Record call expressions
            if (ts.isCallExpression(node)) {
                const sig = checker.getResolvedSignature(node);
                if (sig) {
                    const declaration = sig.declaration;
                    if (declaration && declaration.name) {
                        const calleeQualified = getQualifiedName(declaration, declaration.getSourceFile());
                        const calleeFile = path.relative(repoPath, declaration.getSourceFile().fileName).replace(/\\/g, '/');
                        entity.rawCalls.push({
                            qualifiedName: calleeQualified,
                            file: calleeFile,
                            kind: kindForNode(declaration) || 'function'
                        });
                    }
                }
            }
        }

        ts.forEachChild(node, visit);
    }

    visit(sourceFile);
}

// Post-process: resolve exports, calls, tests, docs into edges
for (const entity of entities) {
    if (entity.rawExported) {
        // Find the file entity and add exports edge
        const fileEntity = entities.find(e => e.kind === 'file' && e.file === entity.file);
        if (fileEntity) {
            edges.push({ src: fileEntity.id, dst: entity.id, kind: 'exports' });
        }
    }

    for (const call of (entity.rawCalls || [])) {
        const callee = entities.find(e =>
            e.qualifiedName === call.qualifiedName && e.file === call.file && e.kind === call.kind
        );
        if (callee) {
            edges.push({ src: entity.id, dst: callee.id, kind: 'calls' });
            // Add dependency relationship
            if (!entity.dependencies.includes(callee.id)) {
                entity.dependencies.push(callee.id);
            }
        }
    }

    if (entity.rawDocComment) {
        const docEntity = entities.find(e => e.qualifiedName === entity.qualifiedName && e.kind === 'doc');
        // Create implicit doc entity and edge
    }
}

// Detect test files and add tests edges
const testFiles = entities.filter(e => e.layer === 'test' && e.kind === 'file');
for (const testEntity of testFiles) {
    // Find all functions called from test files and add tests edges
    const testFunctions = entities.filter(e => e.file === testEntity.file);
    for (const tf of testFunctions) {
        for (const call of (tf.rawCalls || [])) {
            const callee = entities.find(e =>
                e.qualifiedName === call.qualifiedName && e.file === call.file
            );
            if (callee && callee.layer === 'source') {
                edges.push({ src: tf.id, dst: callee.id, kind: 'tests' });
                if (!callee.tests.includes(tf.id)) {
                    callee.tests.push(tf.id);
                }
            }
        }
    }
}

// Clean up internal fields
const cleanEntities = entities.map(e => {
    const { rawExported, rawCalls, rawDocComment, sourceFile, ...clean } = e;
    return clean;
});

// Emit JSON
const output = {
    entities: cleanEntities,
    edges: edges,
};
console.log(JSON.stringify(output));
"#;
