#!/usr/bin/env bash
# Installe la barre de recherche pour l'utilisateur courant, sans sudo : l'AppImage dans
# ~/Applications, le lanceur du menu de Cinnamon avec son icône, le démarrage avec la session,
# puis lance la barre en fond. Relançable sans risque pour installer une nouvelle version.
set -euo pipefail
ici="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
appimage="$HOME/Applications/BarreRecherche.AppImage"

if [[ "${XDG_SESSION_TYPE:-x11}" != "x11" ]]; then
  echo "Attention : la barre ne marche qu'en session X11 (session actuelle : $XDG_SESSION_TYPE)." >&2
fi

# Une barre déjà lancée garderait l'ancienne version : on l'arrête.
if pgrep -x barre-recherche >/dev/null; then
  pkill -x barre-recherche || true
  sleep 1
fi

mkdir -p "$HOME/Applications"
cp "$ici/BarreRecherche.AppImage" "$appimage"
chmod +x "$appimage"

for taille in 32 128 256 512; do
  mkdir -p "$HOME/.local/share/icons/hicolor/${taille}x${taille}/apps"
  cp "$ici/icones/barre-recherche-${taille}.png" "$HOME/.local/share/icons/hicolor/${taille}x${taille}/apps/barre-recherche.png"
done

# Sans FUSE 2, une AppImage ne se monte pas : elle se décompresse alors à chaque lancement.
lancement="\"$appimage\""
if ! ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; then
  echo "FUSE 2 absent : la barre se lancera sans montage (un peu plus lente à démarrer)."
  echo "Pour l'éviter : sudo apt install libfuse2t64"
  lancement="env APPIMAGE_EXTRACT_AND_RUN=1 \"$appimage\""
fi

mkdir -p "$HOME/.local/share/applications"
cat > "$HOME/.local/share/applications/barre-recherche.desktop" <<LANCEUR
[Desktop Entry]
Type=Application
Name=Barre de recherche
Comment=Barre de recherche du poste, ouverte par Ctrl+K
Exec=$lancement
Icon=barre-recherche
Terminal=false
Categories=Utility;
LANCEUR

# Liste des logiciels d'Eloi, la même que sur le fixe : la barre n'y montre que ceux présents ici.
if [[ -f "$ici/logiciels.json" ]]; then
	mkdir -p "$HOME/.config/BarreRecherche"
	cp "$ici/logiciels.json" "$HOME/.config/BarreRecherche/logiciels.json"
fi

# Démarrage avec la session : le même fichier que la case des réglages de la barre.
mkdir -p "$HOME/.config/autostart"
cat > "$HOME/.config/autostart/barre-recherche.desktop" <<DEMARRAGE
[Desktop Entry]
Type=Application
Name=Barre de recherche
Comment=Barre de recherche du poste, ouverte par Ctrl+K
Exec=$lancement --fond
Terminal=false
NoDisplay=true
X-GNOME-Autostart-enabled=true
DEMARRAGE

gtk-update-icon-cache -f -t "$HOME/.local/share/icons/hicolor" >/dev/null 2>&1 || true
update-desktop-database "$HOME/.local/share/applications" >/dev/null 2>&1 || true

# Lancée en fond, détachée du terminal.
if [[ -n "${DISPLAY:-}" ]]; then
  if ldconfig -p 2>/dev/null | grep -q 'libfuse\.so\.2'; then
    setsid -f "$appimage" --fond >/dev/null 2>&1 < /dev/null
  else
    APPIMAGE_EXTRACT_AND_RUN=1 setsid -f "$appimage" --fond >/dev/null 2>&1 < /dev/null
  fi
fi
echo "La barre de recherche est installée : $appimage. Elle démarre avec la session ; Ctrl+K l'ouvre."
