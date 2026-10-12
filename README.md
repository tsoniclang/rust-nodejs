# `@tsonic/rust-nodejs`

Rust Node capability for Tsonic. It owns exact `node:*` provider declarations,
Rust operation rows, installed-runtime contributions, and the
`tsonic_rust_node` crate.

Canonical product documentation:

- [Node capability](https://github.com/tsoniclang/tsonic/blob/main/docs/reference/node-capability.md)
- [Rust Node support](https://github.com/tsoniclang/tsonic/blob/main/docs/reference/targets/rust/node-capability.md)
- [Rust support inventory](https://github.com/tsoniclang/tsonic/blob/main/docs/reference/targets/rust/support-inventory.md)

Install this capability next to the Rust target in the application package:

```sh
npm install --save-dev @tsonic/rust-nodejs@^0.1.0
```

Authored source then imports standard modules such as `node:fs`. Selecting the
JavaScript source surface is a separate choice and is not required for Node
module imports.

## Readable lifecycle and stdin

Readable data and terminal events run on the owning native task queue. The last
`read()` returns its bytes before `end` is delivered; late `end` listeners are
not replayed. `resume()` consumes data even without data listeners. Byte-mode
`readableLength` counts buffered bytes; object mode counts buffered elements.

Native Rust stream factories require the existing typed `TaskHandle<E>`. The
provider supplies it automatically; authored TypeScript requires no scheduler
argument. Native callers retain and poll their `RuntimeTasks<E>` owner.

`process.stdin` has a single typed resource owner per generated runtime context.
Linux pipes and terminals use cancellable nonblocking descriptor readiness;
regular files use the existing bounded background file reader. Other platforms
report `ERR_NODE_STDIN_UNSUPPORTED` instead of blocking the loop or fabricating
flowing support. Readline questions consume their selected Readable; they do not
start a separate standard-input reader.

## Native callback errors

Error callbacks use `NodeError` from `node:util`, a type-only contract with a
readonly `message`. Values retain the actual native error; there is no source
`Error` reconstruction. Compression callbacks expose optional error and output
independently: a failure has no output, while successful empty output is present.

## Develop this capability

```sh
npm install
npm run build
npm test
```

The gate covers provider contracts and the runtime Cargo workspace. Generated
projects bind the canonical runtime crates through explicit installed paths;
physical npm sibling layout is not semantic input.

Generated binaries initialize the performance clock before authored module
initializers. Native Rust embedders must call
`tsonic_rust_node::perf_hooks::initialize_clock()` before using this clock or
calling an emitted library that uses it. Initialization is idempotent; it never
changes the origin of a running clock.
