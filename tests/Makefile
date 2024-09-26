.PHONY: importtest3 clean all
.DELETE_ON_ERROR:

test: all

all: import

import: importtest1 importtest2 importtest3 importtest4

align: aligntest1 aligntest2 aligntest3 aligntest4

clean:
	-rm *test*.annotationstore.stam.json

importtest1.annotationstore.stam.json:
	@echo "------------- Import - Test 1 - Simple parse mode -------------"
	cargo run import --inputfile test1.tsv --annotationset default --resource hello.txt --verbose $@

importtest1: importtest1.annotationstore.stam.json

importtest2.annotationstore.stam.json:
	@echo "------------- Import - Test 2 - Align with text parse mode -------------"
	cargo run import --inputfile test2.tsv --annotationset default --resource hello.txt --verbose $@

importtest2: importtest2.annotationstore.stam.json

importtest3:
	@echo "------------- Import - Test 3 - Simple parse mode with failed validation -------------"
	cargo run import --inputfile test3.tsv --annotationset default --resource hello.txt --verbose importtest3.annotationstore.stam.json; test $$? -ne 0

importtest4: importtest4.annotationstore.stam.json

importtest4.annotationstore.stam.json:
	@echo "------------- Import - Test 4 - Reconstruct text --------------"
	cargo run import --inputfile test2.tsv --annotationset default --new-resource hello2.txt --verbose $@

aligntest1: aligntest1.annotationstore.stam.json

aligntest1.annotationstore.stam.json:
	@echo "------------- Align - Test 1 - Local Alignment (Smith Waterman) --------------"
	cargo run init --force-new  --resource align1.txt --resource localalign1.txt $@
	#oddly enough, this breaks in dev mode where there is an extra check on an unsafe precondition in the underlying seal library, works only in release mode
	cargo run -r align --algorithm local -r localalign1.txt -r align1.txt $@
	cargo run export --alignments $@

aligntest2: aligntest2.annotationstore.stam.json

aligntest2.annotationstore.stam.json:
	@echo "------------- Align - Test 2 - Local Alignment (Smith Waterman) --------------"
	cargo run init --force-new  --resource align1.txt --resource localalign2.txt $@
	cargo run -r align --algorithm local -r localalign2.txt -r align1.txt $@
	cargo run export --alignments $@
	echo "(not a good alignment, this too eagerly matches the first t)"

aligntest4: aligntest4.annotationstore.stam.json

aligntest4.annotationstore.stam.json:
	@echo "------------- Align - Test 2 - Local Alignment (Smith Waterman) --------------"
	cargo run init --force-new  --resource align1.txt --resource localalign2.txt $@
	cargo run -r align --grow --algorithm local -r localalign2.txt -r align1.txt $@
	cargo run export --alignments $@
	echo "(not a good alignment, this too eagerly matches the first t)"

aligntest3: aligntest3.annotationstore.stam.json

aligntest3.annotationstore.stam.json:
	@echo "------------- Align - Test 3 - Global Alignment (Needleman Wunsch) --------------"
	cargo run init --force-new --resource align2.txt --resource localalign2.txt $@
	cargo run -r align --algorithm global -r localalign2.txt -r align2.txt $@
	cargo run export --alignments $@
