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
    --raw-line='#![allow(' \
    --raw-line='    clippy::redundant_static_lifetimes,' \
    --raw-line='    clippy::unreadable_literal,' \
    --raw-line='    clippy::cognitive_complexity' \
    --raw-line=')]' \
    "$@" \
    -- \
    -I include_tmp
}

cp -a ../../vendor/include include_tmp
cp include_tmp/version.h.in include_tmp/version.h

LIBCEC_VERSION_MAJOR=$(grep -E -o 'set\(LIBCEC_VERSION_MAJOR [^)]' ../../vendor/CMakeLists.txt|cut -d ' ' -f2)
LIBCEC_VERSION_MINOR=$(grep -E -o 'set\(LIBCEC_VERSION_MINOR [^)]' ../../vendor/CMakeLists.txt|cut -d ' ' -f2)
LIBCEC_VERSION_PATCH=$(grep -E -o 'set\(LIBCEC_VERSION_PATCH [^)]' ../../vendor/CMakeLists.txt|cut -d ' ' -f2)
sed -i s/@LIBCEC_VERSION_MAJOR@/$LIBCEC_VERSION_MAJOR/ include_tmp/version.h
sed -i s/@LIBCEC_VERSION_MINOR@/$LIBCEC_VERSION_MINOR/ include_tmp/version.h
sed -i s/@LIBCEC_VERSION_PATCH@/$LIBCEC_VERSION_PATCH/ include_tmp/version.h

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
rm -rf include_tmp