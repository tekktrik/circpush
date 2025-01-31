# SPDX-FileCopyrightText: 2025 Alec Delaney
# SPDX-License-Identifier: MIT

.PHONY: test-prep
test-prep:
ifeq "$(OS)" "Windows_NT"
ifeq ($(GITHUB_ACTIONS), "true")
	-@mkdir testmount
	-@xcopy tests\assets\boot_out.txt testmount
	-@subst T: testmount
else
	-xcopy tests\assets\boot_out.txt D:
endif
else ifeq "$(shell uname -s)" "Linux"
	-@truncate testfs -s 1M
	-@mkfs.vfat -F12 -S512 testfs
	-@mkdir testmount
	-@sudo mount -o loop,user,umask=000 testfs testmount/
	-@cp tests/assets/boot_out.txt testmount/
else ifeq "$(shell uname -s)" "Darwin"
	-@hdiutil create -size 512m -volname TESTMOUNT -fs FAT32 testfs.dmg
	-@hdiutil attach testfs.dmg
	-@cp tests/assets/boot_out.txt /Volumes/TESTMOUNT
else
	@echo "Current OS not supported"
	@exit 1
endif

# TODO: This hasn't been tested on Windows
.PHONY: test-run-html
test-run-html:
	cargo llvm-cov --html --features test-support --ignore-filename-regex src/lib.rs

.PHONY: test-run-codecov
test-run-codecov:
	cargo llvm-cov --codecov --output-path target/codecov.json --features test-support --ignore-filename-regex src/lib.rs

.PHONY: test-clean
test-clean:
ifeq "$(OS)" "Windows_NT"
ifneq ($(GITHUB_ACTIONS), "true")
	-@subst T: /d
	-@python scripts\rmdir.py testmount
endif
else ifeq "$(shell uname -s)" "Linux"
	-@sudo umount testmount
	-@sudo rm -rf testmount
	-@rm testfs -f
else ifeq "$(shell uname -s)" "Darwin"
	-@hdiutil detach /Volumes/TESTMOUNT
	-@rm testfs.dmg -f
else
	@echo "Current OS not supported"
	@exit 1
endif

.PHONY: install-dev-reqs
install-dev-reqs:
	-@python -m pip install -r requirements-dev.txt

.PHONY: wipe-test-artifacts
wipe-test-artifacts: install-dev-reqs
	-@python scripts/rmdir_test_config.py

.PHONY: check-test-artifacts
check-test-artifacts: install-dev-reqs
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
	cargo clippy -- --deny warnings

.PHONY: fmt
fmt:
	cargo fmt

.PHONY: reuse
reuse:
	reuse lint

.PHONY: prepare
prepare: reuse clippy fmt test