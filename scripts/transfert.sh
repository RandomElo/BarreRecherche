#!/usr/bin/env bash
# Prépare la barre pour un autre poste : construit l'AppImage et remplit
# <dossier>/BarreRecherche (par défaut ~/Documents/transfert) avec l'AppImage, ses icônes et
# installer.sh, à lancer là-bas. L'AppImage contient les réglages du .env (clé de @bourse) :
# le dossier ne se partage pas.
set -euo pipefail

racine="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dossier="${1:-$HOME/Documents/transfert}/BarreRecherche"

cd "$racine"
npm run package

version="$(node -p "require('./package.json').version")"
appimage="$racine/src-tauri/target/release/bundle/appimage/BarreRecherche_${version}_amd64.AppImage"
if [[ ! -f "$appimage" ]]; then
	echo "AppImage introuvable : $appimage" >&2
	exit 1
fi

mkdir -p "$dossier/icones"
cp "$appimage" "$dossier/BarreRecherche.AppImage"
chmod +x "$dossier/BarreRecherche.AppImage"
for taille in 32 128 256 512; do
	cp "$racine/src-tauri/icons/${taille}x${taille}.png" "$dossier/icones/barre-recherche-${taille}.png"
done
cp "$racine/scripts/transfert/installer.sh" "$dossier/installer.sh"
# La liste des logiciels du fixe (hors dépôt) : sur l'autre poste, seuls ceux présents s'affichent.
if [[ -f "$HOME/.config/BarreRecherche/logiciels.json" ]]; then
	cp "$HOME/.config/BarreRecherche/logiciels.json" "$dossier/logiciels.json"
fi
chmod +x "$dossier/installer.sh"
echo "Prête pour le transfert : $dossier (version $version)."
