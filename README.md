# Barre de recherche

Une barre de recherche pour tout le poste, à la manière de Spotlight. `Ctrl+K` ouvre une petite fenêtre au milieu de l'écran, et on trouve tout au clavier. Le résultat s'ouvre dans le logiciel voulu, avec un aperçu à droite de la ligne choisie.

Pour Linux Mint, bureau Cinnamon, session X11. Le programme est une AppImage autonome : il faut des outils pour la construire, rien pour la faire tourner.

## Ce qu'elle trouve

- Les applications, comme le menu de Cinnamon (« navigateur » trouve Firefox).
- Les pages de Notes, par leur titre, avec leur icône et leur emplacement (les pages parentes).
- Les fichiers et dossiers du dossier personnel, sans les fichiers cachés ni ce qui est généré (`node_modules`, `target`, `venv`…). Parmi les dossiers cachés, seuls ceux de configuration d'un projet sont gardés (`.claude`, `.github`, `.vscode`…), pas ceux du dossier personnel lui-même. L'index se tient à jour tout seul.
- Les marque-pages et l'historique de Firefox, une ligne par site, les plus visités devant.
- Mes skills de Claude Code : « skills claude » ouvre une page locale (recherche, détail au clic, copie de la commande). Elle liste `~/.claude/skills` et se met à jour à chaque ouverture de la barre.
- Les cours de la Bourse et le portefeuille, avec `@bourse` (facultatif, voir l'installation).

La recherche ignore les accents et la casse, et les lettres n'ont pas besoin de se suivre. Les résultats sont rangés en sections : Applications, Notes, Dossiers, Fichiers, Marque-pages, Historique.

L'aperçu montre la ligne active sans l'ouvrir : premières lignes d'un texte ou d'un code, image, première page d'un PDF, page HTML en vignette, contenu d'un dossier, sous-pages et texte d'une page de Notes.

## Installation

La barre se construit depuis ce dossier, puis s'installe dans `~/Applications/BarreRecherche.AppImage`.

### 1. Outils de construction (une seule fois)

- Node.js 22 ou plus : `node -v` pour vérifier.
- Rust, installé par `rustup` dans `~/.cargo`, sans `sudo` :

  ```bash
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
  ```

  Ouvrir ensuite un nouveau terminal, ou faire `source ~/.cargo/env`, pour que `cargo` soit trouvé.

- Les bibliothèques dont la construction a besoin :

  ```bash
  sudo apt install build-essential libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libxdo-dev libssl-dev
  ```

### 2. Dépendances du projet

```bash
npm ci
```

### 3. Réglage de `@bourse` (facultatif)

`@bourse` lit les cours et le portefeuille sur une API bourse personnelle. Sans ce réglage, tout le reste marche, et `@bourse` dit ce qui manque.

```bash
cp .env.example .env
chmod 600 .env
```

Puis remplir `.env` :

```
BOURSE_ADRESSE=http://<adresse-de-l-api>:8140
BOURSE_CLE=<clé de l'API>
# Facultative : la même API par un autre chemin (Tailscale…), essayée si la première ne répond pas.
BOURSE_ADRESSE_SECOURS=http://<autre-adresse>:8140
```

Le `.env` n'est jamais versionné. Ses valeurs sont intégrées à l'AppImage au moment de la construction : après un changement, refaire l'étape 4. L'AppImage contient alors la clé, il ne faut pas la partager.

### 4. Construire et installer

```bash
npm run installer
```

La première construction prend une dizaine de minutes, les suivantes deux ou trois. L'installation ajoute aussi « Barre de recherche » au menu de Cinnamon, avec son icône.

### 5. Lancer, et démarrer avec la session

```bash
~/Applications/BarreRecherche.AppImage
```

La barre s'affiche. Cliquer sur l'engrenage au bout du champ, puis cocher « Démarrer avec la session » : la barre sera lancée en fond, cachée, à chaque ouverture de session, par un service de la session : relancée si elle plante, et si elle plante en boucle, le menu Windows est rendu et une notification le dit.

`Ctrl+K` ouvre ensuite la barre de n'importe où.

## Installer sur un autre poste

Inutile de tout reconstruire là-bas : l'AppImage est autonome. Sur ce poste-ci :

```bash
npm run transfert
```

Il construit l'AppImage et remplit `~/Documents/transfert/BarreRecherche/` (ou `<dossier>/BarreRecherche/` avec `npm run transfert -- <dossier>`) :

- `BarreRecherche.AppImage` ;
- `icones/`, pour le menu ;
- `installer.sh`.

Copier ce dossier sur l'autre poste, puis y lancer :

```bash
bash BarreRecherche/installer.sh
```

Le script, sans `sudo` et relançable pour installer une nouvelle version :

- arrête la barre si elle tourne déjà ;
- copie l'AppImage dans `~/Applications/BarreRecherche.AppImage` ;
- ajoute « Barre de recherche » au menu de Cinnamon, avec son icône ;
- la fait démarrer avec la session ;
- la lance en fond. `Ctrl+K` l'ouvre aussitôt.

Sans FUSE 2 sur ce poste-là, il le dit et lance l'AppImage sans la monter, un peu plus lentement ; `sudo apt install libfuse2t64` l'évite.

L'AppImage contient les réglages du `.env`, dont la clé de `@bourse` : ce dossier ne se partage pas. Sur l'autre poste, `@notes` trouve le coffre réglé dans Notes, et l'ouvre par le lanceur que l'installation de Notes a créé.

## Mettre à jour, désinstaller

Mettre à jour, après un changement du code ou du `.env` :

1. `npm run installer` ;
2. engrenage, puis « Redémarrer » (ou « Quitter », puis la relancer depuis le menu de Cinnamon, « Barre de recherche »). Une barre lancée garde l'ancienne version tant qu'elle n'est pas relancée.

Désinstaller :

1. engrenage, décocher « Démarrer avec la session », puis « Quitter » ;
2. supprimer `~/Applications/BarreRecherche.AppImage`, et, si elle a été installée par `installer.sh`, `~/.local/share/applications/barre-recherche.desktop` ;
3. pour ne rien laisser : supprimer `~/.config/BarreRecherche/` (éléments récents) et `~/.cache/BarreRecherche/` (copie de la base de Firefox, page des skills).

La barre n'écrit nulle part ailleurs, et jamais dans les fichiers qu'elle cherche.

## Utilisation

| Touche | Effet |
|---|---|
| `Ctrl+K` | ouvre la barre, sans filtre ; depuis Notes, avec le filtre Notes |
| loupe de Notes ou `Ctrl+Maj+F` dans Notes | ouvre aussi la barre avec le filtre Notes (Notes lance la barre avec `--cible=notes`) |
| taper | cherche partout : le meilleur résultat en tête, puis toujours les mêmes sections dans le même ordre |
| taper le début d'un filtre (`vs`, `dos`, `no`) | le filtre est proposé en première ligne, mais la ligne choisie est le meilleur résultat : `Entrée` ouvre le logiciel, `Tab` pose le filtre |
| `↑` `↓` | change de ligne, l'aperçu suit |
| `Tab` / `Maj+Tab` | pose le filtre proposé ; sinon entre dans le dossier actif (il devient une étiquette) ; sinon section suivante ou précédente (`Maj+Tab` ne fait que ça) |
| `Entrée` | ouvre la ligne ; sur un filtre ou « Plus de résultats », pose le filtre ; avec un logiciel posé (`vsc`), un dossier en étiquette et le champ vide, ouvre ce dossier dans le logiciel |
| `Windows` (Super) seule | ouvre la barre, comme `Ctrl+K` ; sur la barre ouverte, la ferme ; avec une autre touche (`Super+E`) elle ne fait rien. Tant que la barre tourne, le raccourci du menu de Cinnamon est vidé (applet du menu, réglage `overlay-key`) ; quand elle s'arrête (Quitter, ou plantages en boucle), il est rendu et la touche Windows rouvre le menu. Vider aussi `gsettings set org.cinnamon.muffin overlay-key ''` |
| `→` sur un dossier ou une page de Notes | entre dedans (contenu du dossier, sous-pages de la page) ; `←` remonte |
| champ vide | les filtres de genre (Récents, Logiciels, Applications, Notes, Dossiers…), `Entrée` en pose un |
| champ vide avec un filtre posé | tout ce que le filtre ouvre (Notes : tes pages ; VS Code : les dossiers ouverts récemment dans VS Code ; Dossiers : ton dossier personnel) |
| dernière ligne | « Rechercher sur le Web » dès qu'un texte est tapé, dans un onglet de Firefox ; une adresse tapée (`github.com`) s'ouvre directement |
| `Retour arrière` champ vide | retire la dernière étiquette (dossier parcouru, puis filtre) |
| `Échap` | cache la barre |

Un filtre garde un genre de résultat, ou choisit le logiciel qui l'ouvre. Un genre et un logiciel vont ensemble (VS Code et Fichiers). VS Code et Terminal seuls ne montrent que des dossiers. Un filtre se pose aussi en tapant son nom suivi d'une espace (`vsc rapport`), ou par `@` (`@ter rapport`, `rapport @vsc`).

| Filtre (noms à taper) | Effet |
|---|---|
| Récents, Logiciels, Applications | ce que tu as ouvert dernièrement, tes logiciels, les applications du poste |
| Notes (`notes`) | seulement les pages de Notes, ouvertes dans Notes |
| Dossiers (`dossier`), Fichiers (`fichier`) | seulement les dossiers, ou les fichiers |
| Marque-pages (`favoris`), Historique | pages de Firefox |
| VS Code (`vsc`, `code`) | ouvre le dossier dans VS Code (un fichier avec Fichiers) |
| Terminal (`terminal`, `ghostty`) | ouvre un terminal dans le dossier (Ghostty, sinon le terminal de Cinnamon) |
| Firefox (`firefox`) | pages de Firefox, fichiers HTML et PDF, ouverts dans Firefox |
| Bourse (`bourse`) | le portefeuille, ou le cours d'une action : gain de la journée et courbe du jour |
| Web (`web`, `recherche`) | les pages de Firefox, puis `Entrée` cherche le texte sur le Web, dans un onglet de Firefox, avec son moteur par défaut |
| Google (`google`, `chrome`) | pareil, sur Google, dans Google Chrome |

Exemples :

- `vs`, `Entrée`, `rapport` : les dossiers « rapport », ouverts dans VS Code ;
- `@ter projet` : un terminal ouvert dans le dossier « projet » ;
- `@firefox linkedin` : LinkedIn, depuis les marque-pages ou l'historique ;
- `notes correctifs` : la page « Correctifs » de Notes ;
- `bourse alstom` : le cours d'Alstom, ce qu'il a gagné ou perdu aujourd'hui, et sa courbe.

`Ctrl+K` est pris sur tout le poste, mais les raccourcis en deux temps des logiciels marchent encore. `Ctrl` tenu après `K`, la touche suivante est rendue au logiciel : `Ctrl+K Ctrl+O` ouvre toujours un dossier dans VS Code, même tapé par une touche macro qui relâche `Ctrl` entre les deux. Dans VS Code, la barre s'ouvre donc 300 ms après `Ctrl+K`, le temps de voir si une seconde touche suit.

## En cas de problème

- `Ctrl+K` n'ouvre rien : la barre tourne-t-elle (`pgrep -f BarreRecherche`) ? Sinon, la lancer depuis le menu de Cinnamon. Si `Ctrl+K` est pris par un raccourci de Cinnamon ou d'un autre programme, l'engrenage le dit : libérer le raccourci, puis relancer la barre.
- La barre plante en boucle : après quatre plantages en deux minutes, le service s'arrête, le menu Windows est rendu et une notification le dit. Voir l'erreur : `journalctl --user -u barre-recherche.service`. Relancer : `systemctl --user reset-failed barre-recherche.service`, puis « Barre de recherche » dans le menu de Cinnamon.
- La barre ne démarre pas : la lancer depuis un terminal, `~/Applications/BarreRecherche.AppImage`, pour lire les erreurs.
- `@bourse` affiche un message en jaune : `.env` absent ou incomplet (puis `npm run installer`), clé refusée, ou API injoignable.
- Session Wayland : non prise en charge, se connecter en X11.

## Développement

| Commande | Rôle |
|---|---|
| `npm run dev` | application en développement |
| `npm run check` | typage, lint (ESLint, rustfmt, clippy), tests (Vitest, `cargo test`) |
| `npm run package` | AppImage dans `src-tauri/target/release/bundle/appimage/` |
| `npm run installer` | construit et installe l'AppImage ; `BARRE_DOSSIER_INSTALLATION=<dossier>` pour l'installer ailleurs |
| `npm run transfert` | construit l'AppImage et prépare `~/Documents/transfert/BarreRecherche/` pour un autre poste |
| `npm run version:set -- X.Y.Z` | change la version partout |
| `npm run memoire` | mémoire de la barre lancée |
| `npm run icones` | icônes PNG depuis `src-tauri/icons/icone.svg` |

Tauri 2 : l'interface (`src/`, React, TypeScript, Tailwind) ne parle au système que par les commandes Tauri, et la partie Rust (`src-tauri/src/`) fait tout ce qui touche au disque, au raccourci et aux logiciels. Le raccourci est pris directement sur le serveur X11. Rien ne part sur le réseau, sauf vers l'API de `@bourse` quand cette cible est posée.

La construction laisse un cache de plusieurs Go dans `src-tauri/target/`. `cargo clean --manifest-path src-tauri/Cargo.toml` le vide, au prix d'une construction complète la fois suivante.
