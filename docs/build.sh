#!/bin/sh

cd "$(dirname "$0")"

if [ ! -f './mdbook' ]; then
    curl -L https://github.com/rust-lang/mdBook/releases/download/v0.4.8/mdbook-v0.4.8-x86_64-unknown-linux-gnu.tar.gz | tar zxv
fi

./mdbook build -d dist
cp -Rv static/* dist/
