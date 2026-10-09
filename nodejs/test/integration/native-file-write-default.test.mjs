import assert from "node:assert/strict";
import test from "node:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { compileRust, artifactText } from "../../../../tsonic-rust/test/helpers/rust-session.mjs";
import { runCargo, writeGeneratedProject } from "../../../../tsonic-rust/test/helpers/cargo-projects.mjs";
import { nativeOwnershipCostSupport } from "../../../../tsonic-rust/test/helpers/native-ownership-cost.mjs";
import { assertNoTargetDiagnostics } from "../../../../tsonic/test/scripts/diagnostic-assertions.mjs";
import { createTsonicPlugin } from "../../../dist/index.js";

const source = `
import { writeFileSync } from "node:fs";
import { Buffer } from "node:buffer";
export function writeDefault(path: string, contents: string): void {
  writeFileSync(path, contents);
}
export function writeEncoded(path: string, contents: string, encoding: string): void {
  writeFileSync(path, contents, encoding);
}
export function writeBuffer(path: string, contents: Buffer): void {
  writeFileSync(path, contents);
}
`;

test("default UTF-8 file writes retain borrowed strings and handwritten native costs", { timeout: 300_000 }, () => {
  const { result } = compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
    target: { id: "rust", options: { outputType: "lib", crateName: "native_file_write_default" } },
    files: { "index.ts": source },
  });
  assertNoTargetDiagnostics(result.diagnostics);
  const output = artifactText(result, "src/index.rs");
  assert.match(output, /fn writeDefault\(path: &str, contents: &str\)/u);
  assert.match(output, /write_file_sync_string\(\s*core::convert::AsRef::<str>::as_ref\(path\),\s*core::convert::AsRef::<str>::as_ref\(contents\),\s*"utf8",\s*\)/u);
  assert.match(output, /write_file_sync_string\(\s*core::convert::AsRef::<str>::as_ref\(path\),\s*core::convert::AsRef::<str>::as_ref\(contents\),\s*core::convert::AsRef::<str>::as_ref\(encoding\),\s*\)/u);
  assert.match(output, /write_file_sync_buffer\(\s*core::convert::AsRef::<str>::as_ref\(path\),\s*&contents,\s*\)/u);
  assert.doesNotMatch(output, /\.clone\(|\.to_owned\(|\.to_string\(|String::|Vec::|Box::/u);
  const root = writeGeneratedProject("native-file-write-default", result.artifacts);
  mkdirSync(join(root, "tests"), { recursive: true });
  writeFileSync(join(root, "tests/writes.rs"), `${nativeOwnershipCostSupport}
use native_file_write_default::index;
use tsonic_rust_node::{buffer::Buffer, fs};
use tsonic_rust_runtime::ErrorObject;

#[test]
fn default_encoding_preserves_unicode_and_borrowed_native_costs() {
    let path = "native-default-write.txt";
    for contents in ["", "plain text", "λ€🦀\\0終"] {
        for _ in 0..8 {
            let (native, native_cost) = measure(|| fs::write_file_sync_string(path, contents, "utf8"));
            native.expect("handwritten native write");
            let (generated, generated_cost) = measure(|| index::writeDefault(path, contents));
            generated.expect("generated default UTF-8 write");
            assert_eq!(generated_cost, native_cost);
            assert_eq!(std::fs::read(path).unwrap(), contents.as_bytes());
            let (encoded, encoded_cost) = measure(|| index::writeEncoded(path, contents, "utf8"));
            encoded.expect("generated explicit UTF-8 write");
            assert_eq!(encoded_cost, native_cost);
        }
        let buffer = Buffer::from_string(contents, None).unwrap();
        index::writeBuffer(path, buffer).expect("existing Buffer overload");
        assert_eq!(std::fs::read(path).unwrap(), contents.as_bytes());
    }
    std::fs::remove_file(path).unwrap();
}

#[test]
fn invalid_encoding_and_native_file_errors_preserve_the_provider_boundary() {
    let path = "native-invalid-encoding.txt";
    let expected = fs::write_file_sync_string(path, "contents", "unsupported encoding").unwrap_err();
    let returned = index::writeEncoded(path, "contents", "unsupported encoding").unwrap_err();
    assert_eq!(returned.source_error().error_message(), expected.error_message());
    assert_eq!(returned.source_error().error_name(), expected.error_name());
    assert!(!std::path::Path::new(path).exists());
    let expected = fs::write_file_sync_string("invalid\\0path", "contents", "utf8").unwrap_err();
    let returned = index::writeDefault("invalid\\0path", "contents").unwrap_err();
    assert_eq!(returned.source_error().error_message(), expected.error_message());
    assert_eq!(returned.source_error().error_name(), expected.error_name());
}
`);
  for (const args of [["generate-lockfile", "--offline"], ["fmt", "--all"], ["fmt", "--all", "--check"],
    ["check", "--all-targets", "--locked", "--offline"], ["clippy", "--all-targets", "--locked", "--offline", "--", "-D", "warnings"],
    ["test", "--locked", "--offline"]]) runCargo(root, args);
});

test("file write overloads reject wrong data, path, encoding and argument counts", () => {
  for (const call of [
    "writeFileSync('path')", "writeFileSync('path', 1)", "writeFileSync(1, 'contents')",
    "writeFileSync('path', 'contents', 1)", "writeFileSync('path', 'contents', 'utf8', 'extra')",
    "writeFileSync('path', [1, 2])", "writeFileSync('path', Buffer.alloc(0), 'utf8')",
  ]) {
    assert.throws(() => compileRust({ surfaces: ["js"], capabilities: [createTsonicPlugin()],
      files: { "index.ts": `import { writeFileSync } from 'node:fs'; import { Buffer } from 'node:buffer'; export function invalid(): void { ${call}; }` },
    }), /TypeScript diagnostics:/u, call);
  }
});
