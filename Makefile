# SPDX-FileCopyrightText: 2025 Alec Delaney
# SPDX-License-Identifier: MIT

.PHONY: test-prep
test-prep:
	
ifeq "$(OS)" "Windows_NT"
ifeq "$(CI)" "true"
	xcopy tests\assets\boot_out.txt C:
else
	mkdir testmount
	xcopy tests\assets\boot_out.txt testmount
	subst T: testmount
endif
else ifeq "$(shell uname -s)" "Linux"
	truncate testfs -s 1M
	mkfs.vfat -F12 -S512 testfs
	mkdir testmount
	sudo mount -o loop,user,umask=000 testfs testmount/
	cp tests/assets/boot_out.txt testmount/
else ifeq "$(shell uname -s)" "Darwin"
	hdiutil create -size 512m -volname TESTMOUNT -fs FAT32 testfs.dmg
	hdiutil attach testfs.dmg
	cp tests/assets/boot_out.txt /Volumes/TESTMOUNT
else
	@echo "Current OS not supported"
	@exit 1
endif

.PHONY: test-run-html
test-run-html:
	RUST_TEST_TIME_UNIT=5000,10000 cargo llvm-cov --html --features test-support --ignore-filename-regex src/lib.rs

.PHONY: test-run-codecov
test-run-codecov:
	RUST_TEST_TIME_UNIT=5000,10000 cargo llvm-cov --codecov --output-path target/codecov.json --features test-support --ignore-filename-regex src/lib.rs

.PHONY: test-clean
test-clean:
ifeq "$(OS)" "Windows_NT"
ifneq "$(CI)" "true"
	subst T: /d
	python scripts\rmdir.py testmount
endif
else ifeq "$(shell uname -s)" "Linux"
	sudo umount testmount
	sudo rm -rf testmount
	rm -f testfs
else ifeq "$(shell uname -s)" "Darwin"
	hdiutil detach /Volumes/TESTMOUNT
	rm -f testfs.dmg
else
	@echo "Current OS not supported"
	@exit 1
endif

.PHONY: install-dev-deps
install-dev-deps:
	-@python -m pip install -r requirements-dev.txt

.PHONY: wipe-test-artifacts
wipe-test-artifacts:
	-@python scripts/rmdir_test_config.py

.PHONY: check-test-artifacts
check-test-artifacts:
	@python scripts/check_test_artifacts.py

.PHONY: test
test: check-test-artifacts
	-@"${MAKE}" test-prep --no-print-directory
	-@"${MAKE}" test-run-html --no-print-directory
	-@"${MAKE}" test-clean --no-print-directory

.PHONY: test-codecov
test-codecov: check-test-artifacts
	-@"${MAKE}" test-prep --no-print-directory
	-@"${MAKE}" test-run-codecov --no-print-directory
	-@"${MAKE}" test-clean --no-print-directory

.PHONY: lint
lint:
	@cargo clippy --version
	cargo clippy -- --deny warnings

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: fmt-check
fmt-check:
	cargo fmt --check

.PHONY: reuse
reuse:
	reuse lint

.PHONY: prepare
prepare: reuse fmt lint test