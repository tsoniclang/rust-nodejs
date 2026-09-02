# `@tsonic/rust-nodejs`

Rust Node capability for Tsonic. It owns exact `node:*` provider declarations,
Rust operation rows, installed-runtime contributions, and the
`tsonic_rust_node` crate.

Canonical product documentation:

- [Node capability](https://github.com/tsoniclang/tsonic/blob/main/docs/reference/node-capability.md)
- [Rust Node support](https://github.com/tsoniclang/tsonic/blob/main/docs/reference/targets/rust/node-capability.md)
- [Rust support inventory](https://github.com/tsoniclang/tsonic/blob/main/docs/reference/targets/rust/support-inventory.md)

## Development

```sh
npm install
npm run build
npm test
```

The gate covers provider contracts and the runtime Cargo workspace. Generated
projects bind the canonical runtime crates through explicit installed paths;
physical npm sibling layout is not semantic input.
