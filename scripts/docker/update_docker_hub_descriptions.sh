#!/usr/bin/env bash
set -e

[ -z $1 ] && echo "Give fully qualified image name as parameter"
[ -z $DOCKERHUB_USERNAME ] && echo "Dockerhub username?" && read -s DOCKERHUB_USERNAME
[ -z $DOCKERHUB_PASSWORD ] && echo "Dockerhub password?" && read -s DOCKERHUB_PASSWORD

docker run --rm \
-v $(pwd)/README.md:/data/README.md:ro,Z \
aemdesign/dockerhub-description \
    $DOCKERHUB_USERNAME \
    $DOCKERHUB_PASSWORD \
    $1
    
