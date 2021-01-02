#!/usr/bin/env bash

# Snipped from https://github.com/librespot-org/librespot/blob/dev/contrib/docker-build-pi-armv6hf.sh and edited for libusb
# Originally snipped and tucked from https://github.com/plietar/librespot/pull/202/commits/21549641d39399cbaec0bc92b36c9951d1b87b90
# and further inputs from https://github.com/kingosticks/librespot/commit/c55dd20bd6c7e44dd75ff33185cf50b2d3bd79c3

set -eux
# Get alsa lib and headers
HIDAPI_VER="0.8.0~rc1+git20140818.d17db57+dfsg-2"
LIBUSB_VER="1.0.22-2"
LIBUDEV_VER="241-7~deb10u5+rpi1"
LIBC_VER="2.28-10+rpi1"
LIBRPI_VER="1.20201201-1"
LIBCEC_VER="4.0.4+dfsg1-2+rpi1"
DEPS=( \

  "http://archive.raspberrypi.org/debian/pool/main/r/raspberrypi-firmware/raspberrypi-bootloader_1.20201201-1_armhf.deb" \
  "http://archive.raspberrypi.org/debian/pool/main/r/raspberrypi-firmware/libraspberrypi0_1.20201201-1_armhf.deb" \
  "http://archive.raspberrypi.org/debian/pool/main/libc/libcec/libcec4_4.0.4+dfsg1-2+rpi1+rpt1_armhf.deb" \
  "http://archive.raspberrypi.org/debian/pool/main/libc/libcec/libcec-dev_4.0.4+dfsg1-2+rpi1+rpt1_armhf.deb" \
  "http://archive.raspberrypi.org/debian/pool/main/r/raspberrypi-firmware/raspberrypi-kernel_1.20201201-1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/g/gcc-8/gcc-8-base_8.3.0-6+rpi1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/g/gcc-8/libgcc1_8.3.0-6+rpi1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/g/glibc/libc6_2.28-10+rpi1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libb/libbsd/libbsd0_0.9.1-2_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/g/gcc-8/libstdc++6_8.3.0-6+rpi1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/p/p8-platform/libp8-platform2_2.1.0.1+dfsg1-2_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/s/systemd/libudev1_241-7~deb10u5+rpi1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libxau/libxau6_1.0.8-1+b2_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libxdmcp/libxdmcp6_1.1.2-3_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libxcb/libxcb1_1.13.1-2_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libx11/libx11-data_1.6.7-1+deb10u1_all.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libx11/libx11-6_1.6.7-1+deb10u1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libxext/libxext6_1.3.3-1+b2_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libxrender/libxrender1_0.9.10-1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libx/libxrandr/libxrandr2_1.5.1-1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libu/libunistring/libunistring2_0.9.10-1_armhf.deb" \
  "http://raspbian.raspberrypi.org/raspbian/pool/main/libi/libidn2/libidn2-0_2.0.5-1+deb10u1_armhf.deb" \
  "http://archive.raspberrypi.org/debian/pool/main/r/raspberrypi-firmware/libraspberrypi0_${LIBRPI_VER}_armhf.deb" \
  "http://archive.raspberrypi.org/debian/pool/main/r/raspberrypi-firmware/libraspberrypi-dev_${LIBRPI_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/libc/libcec/libcec-dev_${LIBCEC_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/libc/libcec/libcec4_${LIBCEC_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/h/hidapi/libhidapi-libusb0_${HIDAPI_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/h/hidapi/libhidapi-dev_${HIDAPI_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/libu/libusb-1.0/libusb-1.0-0_${LIBUSB_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/libu/libusb-1.0/libusb-1.0-0-dev_${LIBUSB_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/s/systemd/libudev1_${LIBUDEV_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/s/systemd/libudev-dev_${LIBUDEV_VER}_armhf.deb" \
  "http://mirrordirector.raspbian.org/raspbian/pool/main/g/glibc/libc6_${LIBC_VER}_armhf.deb"
)

# Collect Paths
SYSROOT="/pi-tools/arm-bcm2708/arm-bcm2708hardfp-linux-gnueabi/arm-bcm2708hardfp-linux-gnueabi/sysroot"
CMAKE_SYSROOT=${SYSROOT}
GCC="/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/bin"
GCC_SYSROOT="$GCC/gcc-sysroot"


export PATH=/pi-tools/arm-bcm2708/gcc-linaro-arm-linux-gnueabihf-raspbian-x64/bin/:$PATH
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
echo -e '[target.arm-unknown-linux-gnueabihf]\nlinker = "gcc-sysroot"' > ~/.cargo/config

# Somehow the .cargo/config setting isn't properly read
export RUSTFLAGS="-C linker=gcc-sysroot"

# Overwrite libc and libpthread with the new ones since the sysroot ones are outdated
cp $SYSROOT/lib/arm-linux-gnueabihf/libc-2.28.so $SYSROOT/lib/libc.so.6
cp $SYSROOT/lib/arm-linux-gnueabihf/libpthread-2.28.so $SYSROOT/lib/libpthread.so.0
rm -f $SYSROOT/lib/libstdc++*
ln -sf $SYSROOT/usr/lib/arm-linux-gnueabihf/libstdc++* $SYSROOT/lib/

# Build
cargo build --target arm-unknown-linux-gnueabihf "$@"
