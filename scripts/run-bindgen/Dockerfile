FROM ubuntu

RUN apt-get update \
    && apt-get install -y libcec-dev \
    && bash -c "dpkg-query --show libcec-dev > /usr/include/libcec/libcec-dev_version_info.txt" \
    && tar zcvf /tmp/libcec.tar.gz /usr/include/libcec/
CMD ["cat", "/tmp/libcec.tar.gz"]