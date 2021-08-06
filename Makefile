ZLIBVER = 1.2.11
ZLIB = zlib-$(ZLIBVER)
ZLIBZ = $(ZLIB).tar.gz
XPCOREVER = 1.4.17
XPCORE = xapian-core-$(XPCOREVER)
XPCOREZ = $(XPCORE).tar.xz

build: $(ZLIB) $(XPCORE)/.libs
	cargo build

test: $(ZLIB) $(XPCORE)/.libs
	DYLD_LIBRARY_PATH=$(XPCORE)/.libs cargo test

run: $(ZLIB) $(XPCORE)/.libs
	DYLD_LIBRARY_PATH=$(XPCORE)/.libs cargo run

# Fetch dependencies
$(ZLIBZ):
	wget https://zlib.net/$(ZLIBZ)

$(XPCOREZ):
	wget https://oligarchy.co.uk/xapian/$(XPCOREVER)/$(XPCOREZ)

$(ZLIB): $(ZLIBZ)
	tar -xvzf $(ZLIBZ)
	cd $(ZLIB) \
		&& ./configure \
		&& $(MAKE)

$(XPCORE): $(XPCOREZ)
	tar -xvf $(XPCOREZ)

$(XPCORE)/.libs: $(ZLIB) $(XPCORE)
	# Apply patches to xapian-core from xapian-rusty:
	cp -R xapian-rusty/include $(XPCORE)/.
	cp omenquire.cc $(XPCORE)/api/
	# Build it
	cd $(XPCORE) \
		&& ./configure CPPFLAGS=-I../$(ZLIB) LDFLAGS=-L../$(ZLIB) \
		&& $(MAKE)

clean:
	rm -rf $(XPCORE)
	cargo clean
