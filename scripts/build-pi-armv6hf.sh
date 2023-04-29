#!/usr/bin/env bash

# Snipped from https://github.com/librespot-org/librespot/blob/dev/contrib/docker-build-pi-armv6hf.sh and edited for libusb
# Originally snipped and tucked from https://github.com/plietar/librespot/pull/202/commits/21549641d39399cbaec0bc92b36c9951d1b87b90
# and further inputs from https://github.com/kingosticks/librespot/commit/c55dd20bd6c7e44dd75ff33185cf50b2d3bd79c3

set -eux
# Get alsa lib and headers
HIDAPI_VER="0.8.0~rc1+git20140818.d17db57+dfsg-2"
LIBUSB_VER="1.0.22-2"
LIBUDEV_VER="241-7~deb10u9+rpi1"
LIBC_VER="2.28-10+rpi1+deb10u2"
OPENSSL_VER="1.1.1n-0+deb11u4"
DEPS=( \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/h/hidapi/libhidapi-libusb0_${HIDAPI_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/h/hidapi/libhidapi-dev_${HIDAPI_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/libu/libusb-1.0/libusb-1.0-0_${LIBUSB_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/libu/libusb-1.0/libusb-1.0-0-dev_${LIBUSB_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/s/systemd/libudev1_${LIBUDEV_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/s/systemd/libudev-dev_${LIBUDEV_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/g/glibc/libc6_${LIBC_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/o/openssl/libssl1.1_${OPENSSL_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/o/openssl/libssl-dev_${OPENSSL_VER}_armhf.deb"
)

# Collect Paths
SYSROOT="/pi-tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi/arm-bcm2708hardfp-linux-gnueabi/sysroot"
TOOLCHAIN="/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/"
GCC="$TOOLCHAIN/bin"
GCC_SYSROOT="$GCC/gcc-sysroot"


export PATH=$TOOLCHAIN/bin/:$PATH
export PKG_CONFIG_LIBDIR=${SYSROOT}/usr/lib/arm-linux-gnueabihf/pkgconfig/
export PKG_CONFIG_SYSROOT_DIR=$SYSROOT
export PKG_CONFIG_ALL_STATIC=on

# Link the compiler
export TARGET_CC="$GCC/arm-linux-gnueabihf-gcc"

# Create wrapper around gcc to point to rpi sysroot
echo -e '#!/bin/bash' "\n$TARGET_CC --sysroot $SYSROOT \"\$@\"" > $GCC_SYSROOT
chmod +x $GCC_SYSROOT

if [ ! -f /tmp/sysroot-dl ]; then
  # Add extra target dependencies to our rpi sysroot
  for path in "${DEPS[@]}"; do
    BASE=$(basename $path)
    if [ ! -f ${BASE} ]; then
      curl -OL $path
    fi
    dpkg -x $(basename $path) $SYSROOT
  done
  touch /tmp/sysroot-dl
fi

mkdir -p ~/.cargo/

# point cargo to use gcc wrapper as linker
echo -e '[target.arm-unknown-linux-gnueabihf]\nlinker = "gcc-sysroot"\nstrip = { path = "arm-linux-gnueabihf-strip" }\nobjcopy = { path = "arm-linux-gnueabihf-objcopy" }' > ~/.cargo/config.toml

# Somehow .cargo/config.toml's linker settings are ignored
export RUSTFLAGS="-C linker=gcc-sysroot"
export CC_arm_unknown_linux_gnueabihf=gcc-sysroot
export CARGO_TARGET_arm_unknown_linux_gnueabihf_LINKER="gcc-sysroot"

# fix hidapi build issue
export CFLAGS="-std=c99"

# Overwrite libc and libpthread with the new ones since the sysroot ones are outdated
cp $SYSROOT/lib/arm-linux-gnueabihf/libc-2.28.so $SYSROOT/lib/libc.so.6
cp $SYSROOT/lib/arm-linux-gnueabihf/libpthread-2.28.so $SYSROOT/lib/libpthread.so.0

CMD=$1
shift

# Build
cargo $CMD --target arm-unknown-linux-gnueabihf "$@"

