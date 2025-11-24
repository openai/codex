PREFIX=/usr
DATADIR=$${datarootdir}
DATAROOTDIR=$${prefix}/share

unstable_protocols = \
	unstable/wlr-data-control-unstable-v1.xml \
	unstable/wlr-export-dmabuf-unstable-v1.xml \
	unstable/wlr-foreign-toplevel-management-unstable-v1.xml \
	unstable/wlr-gamma-control-unstable-v1.xml \
	unstable/wlr-input-inhibitor-unstable-v1.xml \
	unstable/wlr-layer-shell-unstable-v1.xml \
	unstable/wlr-output-management-unstable-v1.xml \
	unstable/wlr-output-power-management-unstable-v1.xml \
	unstable/wlr-screencopy-unstable-v1.xml \
	unstable/wlr-virtual-pointer-unstable-v1.xml

check: $(unstable_protocols)
	./check.sh $(unstable_protocols)

clean:
	rm -f wlr-protocols.pc

wlr-protocols.pc: wlr-protocols.pc.in
	sed \
		-e 's:@prefix@:$(PREFIX):g' \
		-e 's:@datadir@:$(DATADIR):g' \
		-e 's:@datarootdir@:$(DATAROOTDIR):g' \
		<$< >$@

install-unstable: $(unstable_protocols)
	mkdir -p $(DESTDIR)$(PREFIX)/share/wlr-protocols/unstable
	for protocol in $^ ; \
	do \
		install -Dm644 $$protocol \
			$(DESTDIR)$(PREFIX)/share/wlr-protocols/$$protocol ; \
	done

install-pc: wlr-protocols.pc
	mkdir -p $(DESTDIR)$(PREFIX)/share/pkgconfig/
	install -Dm644 wlr-protocols.pc \
		$(DESTDIR)$(PREFIX)/share/pkgconfig/wlr-protocols.pc

install: install-unstable install-pc
