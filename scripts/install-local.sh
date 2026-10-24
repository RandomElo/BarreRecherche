#!/usr/bin/env bash
# Construit l'AppImage et l'installe sous un nom fixe, BarreRecherche.AppImage,
# dans ~/Applications (ou le dossier BARRE_DOSSIER_INSTALLATION, s'il est donné).
# Ajoute aussi le lanceur du menu (installation normale seulement).
# Le remplacement se fait par renommage : une barre en cours d'exécution garde son
# ancien fichier jusqu'à ce qu'elle soit relancée.
set -euo pipefail

racine="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
dossier="${BARRE_DOSSIER_INSTALLATION:-$HOME/Applications}"

cd "$racine"
npm run package

version="$(node -p "require('./package.json').version")"
appimage="$racine/src-tauri/target/release/bundle/appimage/BarreRecherche_${version}_amd64.AppImage"
if [[ ! -f "$appimage" ]]; then
	echo "AppImage introuvable : $appimage" >&2
	exit 1
fi

mkdir -p "$dossier"
temporaire="$dossier/.BarreRecherche.AppImage.nouveau"
cp "$appimage" "$temporaire"
chmod +x "$temporaire"
mv -f "$temporaire" "$dossier/BarreRecherche.AppImage"
echo "Installée : $dossier/BarreRecherche.AppImage (version $version)."

# Lanceur du menu de Cinnamon et icônes, seulement pour l'installation normale (pas pour un
# dossier d'essai) : la barre se relance de là si elle a été quittée.
if [[ -z "${BARRE_DOSSIER_INSTALLATION:-}" ]]; then
	for taille in 32 128 256 512; do
		mkdir -p "$HOME/.local/share/icons/hicolor/${taille}x${taille}/apps"
		cp "$racine/src-tauri/icons/${taille}x${taille}.png" "$HOME/.local/share/icons/hicolor/${taille}x${taille}/apps/barre-recherche.png"
	done
	mkdir -p "$HOME/.local/share/applications"
	cat > "$HOME/.local/share/applications/barre-recherche.desktop" <<LANCEUR
[Desktop Entry]
Type=Application
Name=Barre de recherche
Comment=Barre de recherche du poste, ouverte par Ctrl+K
Exec="$dossier/BarreRecherche.AppImage"
Icon=barre-recherche
Terminal=false
Categories=Utility;
LANCEUR
	gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
	update-desktop-database "$HOME/.local/share/applications" >/dev/null 2>&1 || true
	echo "Lanceur ajouté au menu : Barre de recherche."
fi
