# Tsonic Rust Node Runtime

Rust Node.js surface runtime crate for Tsonic-emitted Rust.

The npm artifact `@tsonic/rust-nodejs` owns both the installed Node capability
plugin and the canonical Node runtime source tree. The capability contributes
`rust/crates/tsonic_rust_node`; no copied runtime tree is shipped. The package
requires `@tsonic/rust-js` and `@tsonic/rust-runtime` peers, but their physical
npm locations are independent. The generated Cargo project binds every
canonical crate through explicit installed paths and explicit crates.io source
patch declarations.

## Crate

- Package/crate: `tsonic_rust_node`
