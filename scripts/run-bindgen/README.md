# Scripts to generate FFI bindings

## Prerequisites

- docker
- following folder structure
  - `<root>/cec-rs`
  - `<root>/libcec-sys`

## How to use

```bash
DOCKER="sudo docker" ./bindgen.sh
```

Generates FFI bindings in `<root>/libcec-sys/src/lib.rs`. Bindings are generated from libcec 4.x C API (`cecc.h`).
