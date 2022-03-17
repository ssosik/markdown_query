ZLIBVER = 1.2.11
ZLIB = zlib-$(ZLIBVER)
ZLIBZ = $(ZLIB).tar.gz
XPCOREVER = 1.4.17
XPCORE = xapian-core-$(XPCOREVER)
XPCOREZ = $(XPCORE).tar.xz

.PHONY: build release clean target/debug/mdq target/release/mdq

CARGO ?= cargo

build: target/debug/mdq

tag:
	git tag v`cargo metadata --format-version 1 | jq -r '.packages[] | select(.name =="mdq") | .version'` && \
		git push --tags

release: target/release/mdq

target/debug/mdq: $(ZLIB) $(XPCORE)/.libs
	$(CARGO) build $(TARGET_FLAGS)

target/release/mdq: $(ZLIB) $(XPCORE)/.libs
	$(CARGO) build --release $(TARGET_FLAGS)

test: $(ZLIB) $(XPCORE)/.libs
	$(CARGO) test

run: $(ZLIB) $(XPCORE)/.libs
	$(CARGO) run

clean:
	rm -rf $(ZLIB)
	rm -rf $(XPCORE)
	$(CARGO) clean

# Fetch dependencies
$(ZLIBZ):
	wget https://zlib.net/$(ZLIBZ)

$(XPCOREZ):
	wget https://oligarchy.co.uk/xapian/$(XPCOREVER)/$(XPCOREZ)

$(ZLIB): $(ZLIBZ)
	tar -xvzf $(ZLIBZ)
	cd $(ZLIB) \
		&& ./configure --static \
		&& $(MAKE)

$(XPCORE): $(XPCOREZ)
	tar -xvf $(XPCOREZ)

$(XPCORE)/.libs: $(ZLIB) $(XPCORE)
	# Apply patches to xapian-core from xapian-rusty:
	cp -R xapian-rusty/include $(XPCORE)/.
	cp omenquire.cc $(XPCORE)/api/
	# Build it
	cd $(XPCORE) \
		&& ./configure --enable-static --disable-shared CPPFLAGS=-I../$(ZLIB) LDFLAGS=-L../$(ZLIB) \
		&& $(MAKE)
