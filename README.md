# Barre de recherche

Une barre de recherche pour tout le poste, à la manière de Spotlight. `Ctrl+K` ouvre une petite fenêtre au milieu de l'écran, et on trouve tout au clavier. Le résultat s'ouvre dans le logiciel voulu, avec un aperçu à droite de la ligne choisie.

Pour Linux Mint, bureau Cinnamon, session X11. Le programme est une AppImage autonome : il faut des outils pour la construire, rien pour la faire tourner.

## Ce qu'elle trouve

- Les applications, comme le menu de Cinnamon (« navigateur » trouve Firefox).
- Les pages de Notes, par leur titre, avec leur icône et leur emplacement (les pages parentes).
- Les fichiers et dossiers du dossier personnel, sans les dossiers cachés ni ce qui est généré (`node_modules`, `target`, `venv`…). L'index se tient à jour tout seul.
- Les marque-pages et l'historique de Firefox, une ligne par site, les plus visités devant.
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

La barre s'affiche. Cliquer sur l'engrenage au bout du champ, puis cocher « Démarrer avec la session » : la barre sera lancée en fond, cachée, à chaque ouverture de session.

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

1. quitter la barre : engrenage, puis « Quitter la barre ». Une barre lancée garde l'ancienne version tant qu'elle n'est pas relancée ;
2. `npm run installer` ;
3. relancer la barre depuis le menu de Cinnamon (« Barre de recherche »), ou `~/Applications/BarreRecherche.AppImage --fond`, ou fermer puis rouvrir la session.

Désinstaller :

1. engrenage, décocher « Démarrer avec la session », puis « Quitter la barre » ;
2. supprimer `~/Applications/BarreRecherche.AppImage`, et, si elle a été installée par `installer.sh`, `~/.local/share/applications/barre-recherche.desktop` ;
3. pour ne rien laisser : supprimer `~/.config/BarreRecherche/` (éléments récents) et `~/.cache/BarreRecherche/` (copie de la base de Firefox).

La barre n'écrit nulle part ailleurs, et jamais dans les fichiers qu'elle cherche.

## Utilisation

| Touche | Effet |
|---|---|
| `Ctrl+K` | ouvre la barre, sans cible posée |
| taper | cherche partout, résultats rangés en sections |
| `↑` `↓` | change de ligne, l'aperçu suit |
| `Tab` / `Maj+Tab` | section suivante ou précédente ; pendant que le premier mot se tape (`vs`, `lo`), complète le nom de cible ou `logiciels` montré en grisé, suivi de « filtre … Tab » ; sans `Tab` ni espace, le dernier mot reste une recherche (`dossier vsc` cherche « vsc » parmi les dossiers) |
| `Entrée` | ouvre la ligne ; sur « Plus de résultats », montre toute la section |
| `→` sur un dossier ou une page de Notes | entre dedans (contenu du dossier, sous-pages de la page) ; `←` remonte |
| champ vide, `↓` puis `Entrée` | choisit une catégorie et ne cherche plus que dedans |
| `logiciel ` | tes logiciels (Analyse bancaire, Notes…), ceux présents sur ce poste ; `Entrée` en démarre un |
| `@` | propose les cibles (tableau ci-dessous) ; on peut aussi taper le nom sans `@` en tête du champ, suivi d'une espace (`vsc `) |
| `Entrée` quand rien n'est trouvé | cherche le texte sur le Web, dans un onglet de Firefox (avec peu de résultats, la ligne est en bas de la liste) ; une adresse tapée (`github.com`) s'ouvre directement |
| `Retour arrière` champ vide | remonte d'un dossier, sinon quitte la catégorie, sinon retire la dernière cible |
| `Échap` | cache la barre |

Les cibles choisissent le logiciel qui ouvre le résultat, ou le genre de résultat gardé. Un logiciel et un genre peuvent être posés ensemble (`@vsc @fichier`). `@vsc` et `@terminal` seuls ne montrent que des dossiers.

| Cible | Effet |
|---|---|
| `@vsc` ou `code` | ouvre le dossier dans VS Code (un fichier avec `@fichier`) |
| `@terminal` ou `ghostty` | ouvre un terminal dans le dossier (Ghostty, sinon le terminal de Cinnamon) |
| `@firefox` | pages de Firefox, fichiers HTML et PDF, ouverts dans Firefox |
| `@notes` | seulement les pages de Notes, ouvertes dans Notes |
| `@fichier` | seulement les fichiers |
| `@dossier` | seulement les dossiers |
| `@bourse` | le portefeuille, ou le cours d'une action : gain de la journée et courbe du jour |
| `@web` (ou `recherche`) | rien sur le poste : `Entrée` cherche le texte sur le Web, dans un onglet de Firefox, avec son moteur par défaut |
| `@google` (ou `chrome`) | pareil, sur Google, dans Google Chrome |

Exemples :

- `vsc rapport` : les dossiers « rapport », ouverts dans VS Code ;
- `@ter projet` : un terminal ouvert dans le dossier « projet » (une cible abrégée suivie d'une espace suffit) ;
- `@firefox linkedin` : LinkedIn, depuis les marque-pages ou l'historique ;
- `@notes correctifs` : la page « Correctifs » de Notes ;
- `@bourse alstom` : le cours d'Alstom, ce qu'il a gagné ou perdu aujourd'hui, et sa courbe.

`Ctrl+K` est pris sur tout le poste, mais les raccourcis en deux temps des logiciels marchent encore. `Ctrl` tenu après `K`, la touche suivante est rendue au logiciel : `Ctrl+K Ctrl+O` ouvre toujours un dossier dans VS Code, même tapé par une touche macro qui relâche `Ctrl` entre les deux. Dans VS Code, la barre s'ouvre donc 300 ms après `Ctrl+K`, le temps de voir si une seconde touche suit.

## En cas de problème

- `Ctrl+K` n'ouvre rien : la barre tourne-t-elle (`pgrep -f BarreRecherche`) ? Sinon, la lancer depuis le menu de Cinnamon. Si `Ctrl+K` est pris par un raccourci de Cinnamon ou d'un autre programme, l'engrenage le dit : libérer le raccourci, puis relancer la barre.
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
