## Compiling for Raspberry Pi
This script will produce an armv6 binary suitable for the raspberry pi zero and earlier generation rpi boards. 
To my knowledge it should also work in later versions, but open an issue if you encounter a problem.

```shell
# Build the base docker image
docker build -t rpirust -f Dockerfile.rpi .

# From the project's root
docker run --rm -v (pwd):/src -w /src rpirust ./scripts/build-pi-armv6hf.sh 

# The binary will be in ./target/arm-unknown-linux-gnueabihf/release/minidsp
```
