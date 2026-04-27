# Getting Started

## CLI

### Prebuilt

TODO:
- [ ] Build release in CI/CD for Linux/Mac/Windows and make it available to
download for users to make use of right away

### Build from source

1. Clone the repository
2. Build with `cargo build --release`

### Usage

Run with `./target/release/eldritch --spec spec/encoding_spec.md
   <my_script>.et`

This will generate for each rule:
- [graphviz](https://graphviz.org/) `.dot` diagram of each
one of the rules described, showing all possible histories. The files can be
found in the same directory as the ledger path or in the explicit output
directory.
- serialised filtered signals in the same directory as the input signals stream

## Library

This can also be used as a library, which exposes a FFI compatible with C-ABI,
such that it can be called from most languages. The input will still be a path
to the script or a string input with the script content.

TODO:
- [ ] Expose the bindgen version and use it with Python/Julia
