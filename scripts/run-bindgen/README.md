# Scripts to generate FFI bindings

# Prerequisites

- docker
- following folder structure
  - `root/cec-rs`
  - `root/libec-sys`

# How to use

```bash
DOCKER="sudo docker" ./bindgen.sh
```

Generates FFI bindings in `src/lib.rs` against `cecc.h` header file in `libcec`.