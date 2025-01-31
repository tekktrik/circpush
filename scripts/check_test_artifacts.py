# SPDX-FileCopyrightText: 2025 Alec Delaney
# SPDX-License-Identifier: MIT

"""Cross-platform script for getting the application configuration directory

Author(s): Alec Delaney
"""

# pragma: no cover

import pathlib
import sys

import click

test_config = click.get_app_dir(".circpush-test")
if pathlib.Path(test_config).exists():
    print("Old test artifact exists, please remove before running tests.")
    print("Note that old workspaces and other information may be contained in the folder.")
    print("(You can run make wipe-test-artiacts to remove the test artifacts)")
    sys.exit(1)
