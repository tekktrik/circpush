# SPDX-FileCopyrightText: 2024 Alec Delaney
#
# SPDX-License-Identifier: MIT

.PHONY: test-prep
test-prep:
ifeq "$(OS)" "Windows_NT"
	-@mkdir testmount
	-@xcopy tests\assets\info_uf2.txt testmount
	-@subst T: testmount
else ifeq "$(shell uname -s)" "Linux"
	-@truncate testfs -s 1M
	-@mkfs.vfat -F12 -S512 testfs
	-@mkdir testmount
	-@sudo mount -o loop,user,umask=000 testfs testmount/
	-@cp tests/assets/info_uf2.txt testmount/
else ifeq "$(shell uname -s)" "Darwin"
	-@hdiutil create -size 512m -volname TESTMOUNT -fs FAT32 testfs.dmg
	-@hdiutil attach testfs.dmg
	-@cp tests/assets/info_uf2.txt /Volumes/TESTMOUNT
else
	@echo "Current OS not supported"
	@exit 1
endif

.PHONY:
test-run:
	@if command -v pyenv >/dev/null; \
	then \
		export LD_LIBRARY_PATH=~/.pyenv/versions/3.13.0/lib; \
	fi; \
	cargo llvm-cov --html --jobs 1

.PHONY: test-clean
test-clean:
ifeq "$(OS)" "Windows_NT"
	-@subst T: /d
	-@python scripts/rmdir.py testmount
	-@python scripts/rmdir.py tests/sandbox/circuitpython
else ifeq "$(shell uname -s)" "Linux"
	-@sudo umount testmount
	-@sudo rm -rf testmount
	-@rm testfs -f
	-@rm -rf tests/sandbox/circuitpython
else
	-@hdiutil detach /Volumes/TESTMOUNT
	-@rm testfs.dmg -f
	-@rm -rf tests/sandbox/circuitpython
endif

.PHONY: test
test:
	-@"${MAKE}" test-prep --no-print-directory
	-@"${MAKE}" test-run --no-print-directory
	-@"${MAKE}" test-clean --no-print-directory

.PHONY: lint
lint:
	cargo clippy

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: prepare
prepare: clippy fmt test