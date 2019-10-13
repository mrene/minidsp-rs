#!/usr/bin/env bash
set -ex

OUT=lib.rs
DEST_DIR=../../src/
CEC_REGEX='(libcec|cec|CEC|LIBCEC)_.*'

function generate() {
    bindgen wrapper.h -o ${OUT}.tmp \
    --whitelist-type $CEC_REGEX \
    --whitelist-function $CEC_REGEX \
    --whitelist-var $CEC_REGEX \
    --blacklist-type cec_boolean \
    --no-prepend-enum-name \
    --rustfmt-bindings \
    --raw-line='#![allow(non_upper_case_globals)]' \
    --raw-line='#![allow(non_camel_case_types)]' \
    --raw-line='#![allow(non_snake_case)]' \
    --raw-line='#![allow(dead_code)]' \
    --raw-line='' \
    "$@" \
    -- \
    -I ../../libcec/include
}

# DISABLED --raw-line='#[link(name = "cec")] extern {}' \

# Generate version with enums, and capture the enum definitions
generate --rustified-enum $CEC_REGEX
./sed_bindings.py ${OUT}.tmp ${OUT} --outfile_enum ${OUT}.enum
# Generate (safer) version without enums
generate --constified-enum $CEC_REGEX
./sed_bindings.py ${OUT}.tmp ${OUT}

# Copy enums to cec-rs/src/ crate
cp ${OUT}.enum ../../../cec-rs/src/enums.rs

# Cleanup
rm ${OUT}.tmp ${OUT}.enum
mv ${OUT} ${DEST_DIR}/${OUT}