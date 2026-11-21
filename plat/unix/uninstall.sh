#!/bin/sh
# Takes Klatter out again: its files, and exactly the lines install.sh added to
# Speech Dispatcher's settings.
#
#   ./uninstall.sh               what ./install.sh put in
#   sudo ./uninstall.sh --system what sudo ./install.sh --system put in
set -eu
# Speech Dispatcher's own settings: /etc on Linux, /usr/local/etc on FreeBSD.
SYS_SD=/etc/speech-dispatcher
if [ ! -f "$SYS_SD/speechd.conf" ] && [ -f /usr/local/etc/speech-dispatcher/speechd.conf ]; then
	SYS_SD=/usr/local/etc/speech-dispatcher
fi
if [ "${1:-}" = "--system" ]; then
	PREFIX=/usr/local
	SD=$SYS_SD
else
	PREFIX=$HOME/.local
	SD=${XDG_CONFIG_HOME:-$HOME/.config}/speech-dispatcher
fi
rm -f "$PREFIX/bin/klatter" "$PREFIX/lib/libklatter.so" "$PREFIX/include/klatter.h" "$SD/modules/klatter.conf"
if [ -f "$SD/speechd.conf" ]; then
	grep -v "# Added by Klatter's install.sh$" "$SD/speechd.conf" > "$SD/speechd.conf.klatter-tmp" || true
	mv "$SD/speechd.conf.klatter-tmp" "$SD/speechd.conf"
fi
# Running, it is "speech-dispatch": Linux shortens process names to 15 characters.
if [ "${1:-}" = "--system" ]; then who=""; else who="-u $(id -u)"; fi
pkill $who -x speech-dispatch 2>/dev/null || true
pkill $who -x speech-dispatcher 2>/dev/null || true
echo "Klatter is uninstalled. Your settings in ~/.config/klatter are kept."
