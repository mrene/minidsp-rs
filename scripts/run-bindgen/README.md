# Scripts to generate FFI bindings

## Prerequisites

-   following folder structure
-   `<root>/cec-rs`
-   `<root>/libcec-sys`

## How to use

Run bindgen:

```bash
./bindgen.sh
```

The script generates FFI bindings in `<root>/libcec-sys/src/lib.rs`.
Bindings are generated from libcec 4.x C API (`cecc.h`).

## Updating libcec version

```bash
cd <root>/libcec-sys/libcec
git checkout <tag-or-hash>
cd ../scripts/run-bindgen
./bindgen.sh
cd ../..
cargo build # build
```
