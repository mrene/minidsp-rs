# Scripts to generate FFI bindings

## Prerequisites

- following folder structure
  - `<root>/cec-rs`
  - `<root>/libcec-sys`

## How to use

In `<root>/libcec-sys`:

```bash
cargo build
```

to build `<root>/libcec-sys/libcec`. Actually we are only interested that all the headers are there, specifically `<root>/libcec-sys/libcec/include/version.h`.

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
cd ..
cargo build # generate libcec version.h
cd scripts/run-bindgen
./bindgen.sh
cd ../..
cargo build # try to build again
```
