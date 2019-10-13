TARGETS = aarch64-unknown-linux-gnu arm-unknown-linux-gnueabi armv7-unknown-linux-gnueabihf i686-unknown-linux-gnu i686-unknown-linux-musl mips-unknown-linux-gnu mips64-unknown-linux-gnuabi64 mips64el-unknown-linux-gnuabi64 mipsel-unknown-linux-gnu x86_64-unknown-linux-gnu x86_64-unknown-linux-musl x86_64-pc-windows-gnu
VERSION = 0.1.16



.PHONY: all
all: $(TARGETS) Cross.toml

.PHONY: clean
clean: 
	rm -rf $(TARGETS)
	rm Cross.toml


.PHONY: build 
build:
	for TARGET in $(TARGETS) ; do \
		docker build -t ssalonen/$$TARGET-$(VERSION) $$TARGET ; \
	done

Cross.toml: 
	echo "" > $(@)
	for TARGET in $(TARGETS); do \
		echo "[target.$$TARGET]" >> $(@) ; \
		echo "image=\"ssalonen/$$TARGET-$(VERSION)\"" >> $(@) ; \
		echo "" >> $(@) ; \
	done

$(TARGETS):
	mkdir -p $(@)
	./create_dockerfile.sh $(@) $(VERSION)
