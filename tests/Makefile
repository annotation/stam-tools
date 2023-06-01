.PHONY: test3 clean all

all: test1 test2 test3 test4

clean:
	rm test1.annotationstore.stam.json test2.annotationstore.stam.json test4.annotationstore.stam.json

test1.annotationstore.stam.json:
	@echo "Test 1 - stam import - Simple parse mode"
	../target/debug/stam import --inputfile test1.tsv --annotationset default --resource hello.txt --verbose test1.annotationstore.stam.json

test1: test1.annotationstore.stam.json

test2.annotationstore.stam.json:
	@echo "Test 2 - stam import - Align with text parse mode"
	../target/debug/stam import --inputfile test2.tsv --annotationset default --resource hello.txt --verbose test2.annotationstore.stam.json

test2: test2.annotationstore.stam.json

test3:
	@echo "Test 3 - stam import - Simple parse mode with failed validation"
	../target/debug/stam import --inputfile test3.tsv --annotationset default --resource hello.txt --verbose test3.annotationstore.stam.json; test $$? -ne 0


test4: test4.annotationstore.stam.json

test4.annotationstore.stam.json:
	@echo "Test 2 - stam import - Reconstruct text"
	../target/debug/stam import --inputfile test2.tsv --annotationset default --new-resource hello2.txt --verbose test4.annotationstore.stam.json

