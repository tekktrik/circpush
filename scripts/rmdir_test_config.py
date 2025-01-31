# SPDX-FileCopyrightText: 2025 Alec Delaney
# SPDX-License-Identifier: MIT

"""Cross-platform script for getting the application configuration directory

Author(s): Alec Delaney
"""

# pragma: no cover

import shutil

import click

test_config = click.get_app_dir(".circpush-test")
shutil.rmtree(test_config)
