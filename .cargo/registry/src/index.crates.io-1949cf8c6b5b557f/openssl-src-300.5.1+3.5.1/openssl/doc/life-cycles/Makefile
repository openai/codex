GRAPHS=cipher.dot digest.dot kdf.dot mac.dot pkey.dot rand.dot
IMAGES=

all: png txt
png: $(subst .dot,.png,$(GRAPHS))
txt: $(subst .dot,.txt,$(GRAPHS))
	@echo
	@echo Remember to check and manually fix the mistakes before merging
	@echo into the man pages.
	@echo

# for the dot program:
#	sudo apt install graphviz
%.png: %.dot
	dot -Tpng -O $<
	@mv $<.png $@

# for the graph-easy program:
#	sudo apt install cpanminus
#	sudo cpanm Graph::Easy
%.txt: %.dot
	graph-easy --from=dot --as_ascii < $< > $@

clean:
	rm -f $(wildcard *.png) $(wildcard *.txt)

