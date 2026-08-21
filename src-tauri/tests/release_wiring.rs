use std::fs;
use std::path::PathBuf;

const APP_STATE_IMPL: &str = "impl AppState {";
const INITIALIZE_FUNCTION: &str = "fn initialize_at(";
const NATIVE_CAPTURE: &str = "NativeCaptureCoordinator";
const NATIVE_WORKER_SPAWN: &str = "spawn_native_worker";
const FORBIDDEN_IDENTIFIERS: &[&str] = &["spawn_fail_closed_worker", "run_unavailable_loop"];
const FORBIDDEN_DEFAULT_CAPTURE: &str = "StreamingCapture::default(";

#[test]
#[ignore = "release gate"]
fn release_wiring_contract() {
    let commands_path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/commands.rs");
    let source = fs::read_to_string(&commands_path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", commands_path.display()));
    let normalized = normalize_code(&strip_comments_and_literals(&source));
    let body = initialize_at_body(&normalized);
    let compact: String = body
        .chars()
        .filter(|character| !character.is_whitespace())
        .collect();

    assert!(
        compact.contains(&format!("{NATIVE_CAPTURE}::")),
        "AppState::initialize_at must directly select {NATIVE_CAPTURE}"
    );
    assert!(
        compact.contains(&format!("{NATIVE_WORKER_SPAWN}(")),
        "AppState::initialize_at must directly call {NATIVE_WORKER_SPAWN}"
    );

    for forbidden in FORBIDDEN_IDENTIFIERS {
        assert!(
            !contains_identifier(body, forbidden),
            "AppState::initialize_at must not select {forbidden}"
        );
    }
    assert!(
        !compact.contains(FORBIDDEN_DEFAULT_CAPTURE),
        "AppState::initialize_at must not select StreamingCapture::default"
    );
}

fn initialize_at_body(source: &str) -> &str {
    let impl_start = source
        .find(APP_STATE_IMPL)
        .unwrap_or_else(|| panic!("missing impl AppState"));
    let impl_body = braced_body(&source[impl_start..], "impl AppState");
    let function_start = impl_body
        .find(INITIALIZE_FUNCTION)
        .unwrap_or_else(|| panic!("missing AppState::initialize_at function"));
    braced_body(&impl_body[function_start..], "AppState::initialize_at")
}

fn braced_body<'a>(source: &'a str, context: &str) -> &'a str {
    let body_start = source
        .find('{')
        .unwrap_or_else(|| panic!("missing {context} body"));

    let mut depth = 0usize;
    for (offset, character) in source[body_start..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth = depth
                    .checked_sub(1)
                    .unwrap_or_else(|| panic!("unbalanced {context} body"));
                if depth == 0 {
                    return &source[body_start + 1..body_start + offset];
                }
            }
            _ => {}
        }
    }
    panic!("unterminated {context} body")
}

fn contains_identifier(source: &str, identifier: &str) -> bool {
    source.match_indices(identifier).any(|(index, _)| {
        let before = source[..index].chars().next_back();
        let after = source[index + identifier.len()..].chars().next();
        before.is_none_or(|character| !is_identifier_character(character))
            && after.is_none_or(|character| !is_identifier_character(character))
    })
}

fn is_identifier_character(character: char) -> bool {
    character == '_' || character.is_ascii_alphanumeric()
}

fn normalize_code(source: &str) -> String {
    source.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn strip_comments_and_literals(source: &str) -> String {
    let bytes = source.as_bytes();
    let mut output = String::with_capacity(source.len());
    let mut index = 0usize;
    let mut block_depth = 0usize;

    while index < bytes.len() {
        if block_depth > 0 {
            if bytes.get(index..index + 2) == Some(b"/*") {
                block_depth += 1;
                index += 2;
            } else if bytes.get(index..index + 2) == Some(b"*/") {
                block_depth -= 1;
                index += 2;
            } else {
                if bytes[index] == b'\n' {
                    output.push('\n');
                }
                index += 1;
            }
            continue;
        }

        if bytes.get(index..index + 2) == Some(b"//") {
            while index < bytes.len() && bytes[index] != b'\n' {
                index += 1;
            }
            continue;
        }
        if bytes.get(index..index + 2) == Some(b"/*") {
            block_depth = 1;
            index += 2;
            continue;
        }
        if bytes[index] == b'"' || bytes[index] == b'\'' {
            let delimiter = bytes[index];
            output.push(' ');
            index += 1;
            while index < bytes.len() {
                if bytes[index] == b'\\' {
                    index += 2;
                } else if bytes[index] == delimiter {
                    index += 1;
                    break;
                } else {
                    if bytes[index] == b'\n' {
                        output.push('\n');
                    }
                    index += 1;
                }
            }
            continue;
        }

        output.push(bytes[index] as char);
        index += 1;
    }

    output
}
