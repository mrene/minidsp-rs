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
    --no-prepend-enum-name \
    --rustfmt-bindings \
    --raw-line='#![allow(non_upper_case_globals)]' \
    --raw-line='#![allow(non_camel_case_types)]' \
    --raw-line='#![allow(non_snake_case)]' \
    --raw-line='#![allow(dead_code)]' \
    --raw-line='' \
    --raw-line='#[link(name = "cec")] extern {}' \
    "$@" \
    -- \
    -I /usr/include/libcec/
}

# Generate 
generate --rustified-enum $CEC_REGEX
#cat ${OUT}.tmp > ${OUT}.tmp1
./sed_bindings.py ${OUT}.tmp ${OUT} --outfile_enum ${OUT}.enum
generate --constified-enum $CEC_REGEX
./sed_bindings.py ${OUT}.tmp ${OUT}
cat ${OUT}.enum >> ${OUT}
rm ${OUT}.tmp ${OUT}.enum
mv ${OUT} ${DEST_DIR}/${OUT}


#     --rustified-enum $CEC_REGEX \
#    --constified-enum $CEC_REGEX \