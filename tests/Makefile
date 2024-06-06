.PHONY: importtest3 clean all

test: all

all: test_import

test_import: importtest1 importtest2 importtest3 importtest4

clean:
	-rm importtest*.annotationstore.stam.json

importtest1.annotationstore.stam.json:
	@echo "Import - Test 1 - Simple parse mode"
	cargo run import --inputfile test1.tsv --annotationset default --resource hello.txt --verbose $@

importtest1: importtest1.annotationstore.stam.json

importtest2.annotationstore.stam.json:
	@echo "Import - Test 2 - Align with text parse mode"
	cargo run import --inputfile test2.tsv --annotationset default --resource hello.txt --verbose $@

importtest2: importtest2.annotationstore.stam.json

importtest3:
	@echo "Import - Test 3 - Simple parse mode with failed validation"
	cargo run import --inputfile test3.tsv --annotationset default --resource hello.txt --verbose importtest3.annotationstore.stam.json; test $$? -ne 0

importtest4: importtest4.annotationstore.stam.json

importtest4.annotationstore.stam.json:
	@echo "Import - Test 4 - Reconstruct text"
	cargo run import --inputfile test2.tsv --annotationset default --new-resource hello2.txt --verbose $@


