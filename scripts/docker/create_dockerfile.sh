cat << EOF > $1/Dockerfile
FROM rustembedded/cross:$1-$2

RUN apt-get remove -y libudev1 udev libudev-dev || :
EOF
