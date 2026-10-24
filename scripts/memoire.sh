#!/usr/bin/env bash
# Mémoire de la barre en cours d'exécution, en PSS (la mémoire partagée comptée une seule fois) :
# le programme et ses processus WebKit. Ne fait que lire /proc. Usage : npm run memoire
set -euo pipefail

principal="$(pgrep -o -x barre-recherche || true)"
if [[ -z "$principal" ]]; then
	echo "La barre ne tourne pas." >&2
	exit 1
fi

descendants() {
	echo "$1"
	for enfant in $(ps -o pid= --ppid "$1"); do
		descendants "$enfant"
	done
}

total=0
for pid in $(descendants "$principal"); do
	pss="$(awk '/^Pss:/ { print $2 }' "/proc/$pid/smaps_rollup" 2>/dev/null || true)"
	[[ -n "$pss" ]] || continue
	nom="$(basename "$(tr '\0' '\n' <"/proc/$pid/cmdline" | head -n 1)")"
	printf '  %-28s %4d Mo\n' "$nom" $((pss / 1024))
	total=$((total + pss))
done
printf 'Total : %d Mo\n' $((total / 1024))
