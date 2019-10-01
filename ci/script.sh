# This script takes care of testing your crate

set -ex

# This is the "test phase", tweak it as you see fit
main() {
    # Build custom docker images for the cross build
    docker build -t cross-libcec-sys:armv7-unknown-linux-gnueabihf-0.1.16 cross_targets/Dockerfile.armv7-unknown-linux-gnueabihf
    docker build -t cross-libcec-sys:x86_64-unknown-linux-gnu-0.1.16 cross_targets/Dockerfile.x86_64-unknown-linux-gnu

    cross build --target $TARGET
    cross build --target $TARGET --release

    if [ ! -z $DISABLE_TESTS ]; then
        return
    fi

    cross test --target $TARGET
    cross test --target $TARGET --release

    cross run --target $TARGET
    cross run --target $TARGET --release
}

# we don't run the "test phase" when doing deploys
if [ -z $TRAVIS_TAG ]; then
    main
fi
