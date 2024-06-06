.PHONY: importtest3 clean all

test: all

all: test_import

test_import: importtest1 importtest2 importtest3 importtest4

clean:
	-rm importtest*.annotationstore.stam.json

importtest1.annotationstore.stam.json:
	@echo "Test 1 - stam import - Simple parse mode"
	cargo run import --inputfile test1.tsv --annotationset default --resource hello.txt --verbose $@

importtest1: test1.annotationstore.stam.json

importtest2.annotationstore.stam.json:
	@echo "Test 2 - stam import - Align with text parse mode"
	cargo run import --inputfile test2.tsv --annotationset default --resource hello.txt --verbose $@

importtest2: test2.annotationstore.stam.json

importtest3:
	@echo "Test 3 - stam import - Simple parse mode with failed validation"
	cargo run import --inputfile test3.tsv --annotationset default --resource hello.txt --verbose importtest3.annotationstore.stam.json; test $$? -ne 0

importtest4: test4.annotationstore.stam.json

importtest4.annotationstore.stam.json:
	@echo "Test 4 - stam import - Reconstruct text"
	cargo run import --inputfile test2.tsv --annotationset default --new-resource hello2.txt --verbose $@


