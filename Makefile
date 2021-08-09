ZLIBVER = 1.2.11
ZLIB = zlib-$(ZLIBVER)
ZLIBZ = $(ZLIB).tar.gz
XPCOREVER = 1.4.17
XPCORE = xapian-core-$(XPCOREVER)
XPCOREZ = $(XPCORE).tar.xz

build: target/debug/tika

release: target/release/tika

target/debug/tika: $(ZLIB) $(XPCORE)/.libs
	cargo build

target/release/tika: $(ZLIB) $(XPCORE)/.libs
	cargo build --release

test:
	cargo test

run: $(ZLIB) $(XPCORE)/.libs
	cargo run

clean:
	rm -rf $(ZLIB)
	rm -rf $(XPCORE)
	cargo clean

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
		&& ./configure --enable-static CPPFLAGS=-I../$(ZLIB) LDFLAGS=-L../$(ZLIB) \
		&& $(MAKE)
