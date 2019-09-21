#!/usr/bin/env bash
set -ex
OUT=bindings.rs
DEST_DIR=../../src/
CEC_REGEX='(libcec|cec|CEC|LIBCEC)_.*'
bindgen wrapper.h -o ${OUT}.tmp \
    --whitelist-type $CEC_REGEX \
    --whitelist-function $CEC_REGEX \
    --whitelist-var $CEC_REGEX \
    --rustified-enum $CEC_REGEX \
    --no-prepend-enum-name \
    --rustfmt-bindings \
    --raw-line='#![allow(non_upper_case_globals)]' \
    --raw-line='#![allow(non_camel_case_types)]' \
    --raw-line='#![allow(non_snake_case)]' \
    --raw-line='#![allow(dead_code)]' \
    --raw-line='' \
    --raw-line='#[link(name = "cec")] extern {}' \
    -- \
    -I /usr/include/libcec/
./sed_bindings.py ${OUT}.tmp ${OUT}
rm ${OUT}.tmp
mv ${OUT} ${DEST_DIR}/${OUT}
