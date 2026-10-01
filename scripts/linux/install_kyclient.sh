#!/bin/bash
# Project Kyber: install_kyclient.sh
# Copyright © 2022-2026 Kyber SAS
# SPDX-License-Identifier: LicenseRef-Kyber-Commercial OR AGPL-3.0-or-later
#
# This file is both under dual license: AGPLv3 and a Commercial one.
#
# ----
# This program is free software: you can redistribute it and/or modify
# it under the terms of the GNU Affero General Public License as published by
# the Free Software Foundation, either version 3 of the License, or
# (at your option) any later version.
#
# This program is distributed in the hope that it will be useful,
# but WITHOUT ANY WARRANTY; without even the implied warranty of
# MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
# GNU Affero General Public License for more details.
#
# You should have received a copy of the GNU Affero General Public License
# along with this program.  If not, see <http://www.gnu.org/licenses/>.

# Register the kyber:// URL scheme so clicking a portal link launches the
# client. Run once after extracting the package.
set -e

BASE_DIR=$(realpath "$(dirname "$0")")
DESKTOP_SRC="$BASE_DIR/share/applications/kyber.desktop"
DEST_DIR="$HOME/.local/share/applications"

if [ ! -f "$DESKTOP_SRC" ]; then
    echo "Error: $DESKTOP_SRC not found" >&2
    exit 1
fi

mkdir -p "$DEST_DIR"

# Point Exec at the launcher wrapper so PATH and LD_LIBRARY_PATH are set when
# the desktop environment starts the client. Inside a quoted Exec argument the
# Desktop Entry spec escapes `"`, backtick, `$` and `\` with a backslash, and
# a literal `%` as `%%`.
exec_dir=$(printf '%s' "$BASE_DIR" | sed -e 's/[\\"`$]/\\&/g' -e 's/%/%%/g')
grep -v '^Exec=' "$DESKTOP_SRC" > "$DEST_DIR/kyber.desktop"
printf 'Exec="%s/run_kyclient.sh" %%u\n' "$exec_dir" >> "$DEST_DIR/kyber.desktop"

update-desktop-database "$DEST_DIR" 2>/dev/null || true
xdg-mime default kyber.desktop x-scheme-handler/kyber

echo "Registered kyber:// scheme handler."
