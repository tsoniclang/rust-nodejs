use std::path::{Path, PathBuf};

#[path = "../../../../rust-runtime/tests/architecture/source_scope.rs"]
mod source_scope;

const FORBIDDEN_PATTERNS: &[&str] = &[
    "quickjs",
    "rquickjs",
    "extern crate v8",
    "use v8::",
    "boa_engine",
    "std::process::Command::new(\"node\")",
    "std::process::Command::new(\"npm\")",
    "std::process::Command::new(\"npx\")",
    "std::process::Command::new(\"tsx\")",
    "Command::new(\"node\")",
    "Command::new(\"npm\")",
    "Command::new(\"npx\")",
    "Command::new(\"tsx\")",
    "std::any::Any",
    "TypeId",
    "downcast",
];

#[test]
fn no_forbidden_shortcuts_present_in_product_sources() {
    let workspace_root = locate_workspace_root()
        .unwrap_or_else(|| panic!("unable to locate workspace root from test manifest directory"));
    let mut violations = Vec::new();
    let sources = source_scope::collect_production_sources(&workspace_root.join("crates"))
        .unwrap_or_else(|error| panic!("unable to classify native product sources: {error}"));

    for source in sources {
        for forbidden in find_forbidden_patterns(&source.source) {
            violations.push(format!("{}: contains `{}`", source.path.display(), forbidden));
        }
    }

    if !violations.is_empty() {
        let mut message = String::from("forbidden shortcuts found:\n");
        for violation in violations {
            message.push_str(" - ");
            message.push_str(&violation);
            message.push('\n');
        }
        panic!("{}", message);
    }
}

#[test]
fn no_forbidden_shortcuts_in_fixture_text() {
    let source = r#"
        let code = std::process::Command::new("node").arg("--version").spawn();
    "#;
    let hits = find_forbidden_patterns(source);
    assert!(!hits.is_empty());
    assert!(!find_forbidden_patterns("use v8::Isolate;").is_empty());
    assert!(!find_forbidden_patterns("extern crate v8 as engine;").is_empty());
}

#[test]
fn allowlisted_name_occurrences_are_not_flagged_by_scanner() {
    let source = r#"
        use tsonic_rust_node::error::NodeError;
        let kind = "node";
        let module = "tsonic_rust_node";
        let node_error = NodeError::new("E001", "node sample");
        let class = "NodeError";
        pub mod v8;
        let unsupported = "node:v8 requires a V8 engine";
        assert!(!kind.is_empty() && !module.is_empty() && !class.is_empty());
        assert!(!node_error.code().is_empty());
    "#;
    let hits = find_forbidden_patterns(source);
    assert!(hits.is_empty());
}

fn find_forbidden_patterns(source: &str) -> Vec<&'static str> {
    FORBIDDEN_PATTERNS
        .iter()
        .copied()
        .filter(|pattern| source.contains(pattern))
        .collect()
}

fn locate_workspace_root() -> Option<PathBuf> {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut current = manifest_dir.to_path_buf();
    loop {
        if current.join("crates").is_dir() && current.join("tests").is_dir() {
            return Some(current);
        }

        if let Some(parent) = current.parent() {
            current = parent.to_path_buf();
            continue;
        }
        return None;
    }
}

