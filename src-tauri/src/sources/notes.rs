//! Pages de Notes (spécification §7) : lues dans le coffre de Notes (`vaultLocal` de
//! `~/.config/Notes/settings.json`), sans jamais y écrire (règle 2). Une page est un dossier qui
//! contient un `page.md` dont le front-matter YAML donne `id` (UUID) et `title` ; ses sous-pages
//! sont ses sous-dossiers. Même lecture que `Notes/packages/core/src/vault/tree.ts` : noms cachés
//! et dossiers `assets` ignorés, page sans front-matter valide (non importée) laissée de côté.

use std::fs;
use std::path::{Path, PathBuf};

/// Fichier qui porte le contenu d'une page dans son dossier.
pub const FICHIER_PAGE: &str = "page.md";
/// Garde-fous du parcours : profondeur et nombre de pages.
const PROFONDEUR_MAX: usize = 32;
const PAGES_MAX: usize = 20_000;

#[derive(Debug, Clone, PartialEq)]
pub struct PageNotes {
    /// UUID de la page, en minuscules : ce que Notes reçoit pour l'ouvrir.
    pub id: String,
    pub titre: String,
    /// Titres des pages parentes, de la racine à la page (« ESGI › Cours »), vide à la racine.
    pub emplacement: String,
    /// Le `page.md`, pour l'aperçu.
    pub fichier: PathBuf,
    pub icone: Option<IconeNote>,
    /// Page parente la plus proche (`→` y mène depuis elle), `None` à la racine.
    pub parent: Option<String>,
    /// La page a des sous-pages : `→` entre dedans (spécification §6).
    pub a_des_enfants: bool,
}

/// Icône d'une page, clé `icon` du front-matter, comme `natureDIcone` de Notes
/// (`packages/core/src/page/icon.ts`).
#[derive(Debug, Clone, PartialEq)]
pub enum IconeNote {
    /// Écrit tel quel (`icon: 📚`).
    Emoji(String),
    /// Nom d'icône Lucide en kebab-case (`icon: book-open`).
    Lucide(String),
    /// Image du dossier `assets/` de la page : son chemin absolu.
    Image(PathBuf),
}

/// Extensions d'une icône image.
const EXTENSIONS_IMAGE: [&str; 7] = ["webp", "png", "jpg", "jpeg", "gif", "avif", "svg"];

/// Forme de la valeur `icon`, ou `None` si elle n'en a aucune. `dossier` : celui de la page.
pub fn nature_icone(valeur: &str, dossier: &Path) -> Option<IconeNote> {
    let icone = valeur.trim();
    if icone.is_empty() || icone.chars().count() > 120 || icone.contains(['\n', '\r']) {
        return None;
    }
    if let Some(nom) = icone.strip_prefix("assets/") {
        let extension = Path::new(nom).extension()?.to_str()?.to_ascii_lowercase();
        let image = !nom.contains(['/', '\\'])
            && !icone.contains("..")
            && EXTENSIONS_IMAGE.contains(&extension.as_str());
        return image.then(|| IconeNote::Image(dossier.join("assets").join(nom)));
    }
    let lucide = icone.split('-').all(|m| {
        !m.is_empty()
            && m.chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
    });
    if lucide {
        return Some(IconeNote::Lucide(icone.to_string()));
    }
    // Un emoji : au moins un caractère hors ASCII, court, sans espace.
    let emoji =
        !icone.is_ascii() && icone.chars().count() <= 16 && !icone.contains(char::is_whitespace);
    emoji.then(|| IconeNote::Emoji(icone.to_string()))
}

/// Ce que la barre lit dans le front-matter d'une page.
#[derive(Debug, Clone, PartialEq)]
pub struct Meta {
    pub id: String,
    pub titre: String,
    /// Valeur brute de `icon`.
    pub icone: Option<String>,
}

/// Dossier des réglages de Notes : `$XDG_CONFIG_HOME/Notes`, sinon `~/.config/Notes`.
fn dossier_reglages() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("Notes"))
}

/// Coffre de Notes, s'il est réglé et existe.
pub fn coffre() -> Option<PathBuf> {
    let texte = fs::read_to_string(dossier_reglages()?.join("settings.json")).ok()?;
    let reglages: serde_json::Value = serde_json::from_str(&texte).ok()?;
    let chemin = PathBuf::from(reglages["vaultLocal"].as_str()?);
    (chemin.is_absolute() && chemin.is_dir()).then_some(chemin)
}

/// Toutes les pages importées du coffre, dans l'ordre de Notes : chaque page suivie de ses
/// sous-pages, les sœurs dans l'ordre de leur `.order` puis par titre.
pub fn lire_pages(coffre: &Path) -> Vec<PageNotes> {
    let mut pages = Vec::new();
    parcourir(coffre, &mut Vec::new(), None, 0, &mut pages);
    let parents: std::collections::HashSet<String> =
        pages.iter().filter_map(|p| p.parent.clone()).collect();
    for page in &mut pages {
        page.a_des_enfants = parents.contains(&page.id);
    }
    pages
}

/// Identifiants du `.order` d'un dossier, dans l'ordre (comme `lireOrdre` de Notes).
fn lire_ordre(dossier: &Path) -> Vec<String> {
    let mut vus = Vec::new();
    for ligne in fs::read_to_string(dossier.join(".order"))
        .unwrap_or_default()
        .lines()
    {
        let id = ligne.trim();
        if est_uuid(id) && !vus.iter().any(|v| v == id) {
            vus.push(id.to_string());
        }
    }
    vus
}

/// `parents` : titres de la racine au dossier ; `parent` : la page la plus proche au-dessus.
fn parcourir(
    dossier: &Path,
    parents: &mut Vec<String>,
    parent: Option<&str>,
    profondeur: usize,
    pages: &mut Vec<PageNotes>,
) {
    if profondeur > PROFONDEUR_MAX || pages.len() >= PAGES_MAX {
        return;
    }
    let Ok(entrees) = fs::read_dir(dossier) else {
        return;
    };
    let mut enfants: Vec<(PathBuf, Option<Meta>)> = entrees
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_dir()))
        .filter(|e| {
            let nom = e.file_name();
            let nom = nom.to_string_lossy();
            !nom.starts_with('.') && nom != "assets"
        })
        .map(|e| {
            let meta = fs::read_to_string(e.path().join(FICHIER_PAGE))
                .ok()
                .and_then(|texte| lire_meta(&texte));
            (e.path(), meta)
        })
        .collect();
    // Ordre de Notes : d'abord les pages du `.order`, dans cet ordre, puis les autres par titre.
    let ordre = lire_ordre(dossier);
    let titre_de = |(chemin, meta): &(PathBuf, Option<Meta>)| match meta {
        Some(meta) => meta.titre.clone(),
        None => chemin
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
    };
    enfants.sort_by_cached_key(|enfant| {
        let rang = enfant
            .1
            .as_ref()
            .and_then(|m| ordre.iter().position(|id| *id == m.id))
            .unwrap_or(usize::MAX);
        (
            rang,
            crate::floue::normaliser_en_chaine(&titre_de(enfant)),
            enfant.0.clone(),
        )
    });
    for (sous_dossier, meta) in enfants {
        match meta {
            Some(Meta { id, titre, icone }) => {
                pages.push(PageNotes {
                    id: id.clone(),
                    titre: titre.clone(),
                    emplacement: parents.join(" › "),
                    fichier: sous_dossier.join(FICHIER_PAGE),
                    icone: icone.and_then(|i| nature_icone(&i, &sous_dossier)),
                    parent: parent.map(String::from),
                    a_des_enfants: false,
                });
                parents.push(titre);
                parcourir(&sous_dossier, parents, Some(&id), profondeur + 1, pages);
                parents.pop();
            }
            // Dossier sans page, ou page non importée : ses sous-pages restent cherchées, sous le
            // nom du dossier, rattachées à la page au-dessus.
            None => {
                parents.push(
                    sous_dossier
                        .file_name()
                        .unwrap_or_default()
                        .to_string_lossy()
                        .into_owned(),
                );
                parcourir(&sous_dossier, parents, parent, profondeur + 1, pages);
                parents.pop();
            }
        }
    }
}

/// Front-matter d'un `page.md` : le bloc entre une première ligne `---` et la ligne `---` suivante.
pub fn decouper(texte: &str) -> Option<(&str, &str)> {
    let texte = texte.strip_prefix('\u{feff}').unwrap_or(texte);
    let reste = texte
        .strip_prefix("---\n")
        .or_else(|| texte.strip_prefix("---\r\n"))?;
    let mut position = 0;
    for ligne in reste.split_inclusive('\n') {
        if ligne.trim_end_matches(['\r', '\n']) == "---" {
            return Some((&reste[..position], &reste[position + ligne.len()..]));
        }
        position += ligne.len();
    }
    None
}

/// `id`, `title` et `icon` du front-matter, ou `None` si la page n'est pas importée (front-matter
/// absent, identifiant qui n'est pas un UUID, titre vide). Seules les valeurs simples sur une ligne
/// sont lues (nues, entre `'…'` ou `"…"`), comme Notes les écrit.
pub fn lire_meta(texte: &str) -> Option<Meta> {
    let (en_tete, _) = decouper(texte)?;
    let mut id = None;
    let mut titre = None;
    let mut icone = None;
    for ligne in en_tete.lines() {
        if ligne.starts_with([' ', '\t']) {
            continue;
        }
        let Some((cle, valeur)) = ligne.split_once(':') else {
            continue;
        };
        match cle.trim() {
            "id" => id = valeur_simple(valeur),
            "title" => titre = valeur_simple(valeur),
            "icon" => icone = valeur_simple(valeur),
            _ => {}
        }
    }
    let id = id.filter(|i| est_uuid(i))?;
    let titre = titre
        .map(|t| t.trim().to_string())
        .filter(|t| !t.is_empty())?;
    Some(Meta { id, titre, icone })
}

/// Valeur YAML simple sur une ligne.
fn valeur_simple(brute: &str) -> Option<String> {
    let valeur = brute.trim();
    if let Some(interieur) = valeur.strip_prefix('\'') {
        return Some(interieur.strip_suffix('\'')?.replace("''", "'"));
    }
    if valeur.starts_with('"') {
        // Jusqu'au guillemet fermant (un commentaire peut suivre). Les échappements YAML courants
        // entre guillemets doubles sont ceux de JSON.
        let mut echappe = false;
        let fin = valeur.char_indices().skip(1).find_map(|(i, c)| {
            let ferme = c == '"' && !echappe;
            echappe = c == '\\' && !echappe;
            ferme.then_some(i)
        })?;
        return serde_json::from_str::<String>(&valeur[..=fin]).ok();
    }
    // Valeur nue : un commentaire commence par « # » après une espace.
    let valeur = valeur.split(" #").next().unwrap_or("").trim();
    (!valeur.is_empty()).then(|| valeur.to_string())
}

/// UUID en minuscules avec tirets (8-4-4-4-12), comme `estId` de Notes.
pub fn est_uuid(texte: &str) -> bool {
    let morceaux: Vec<&str> = texte.split('-').collect();
    morceaux.len() == 5
        && morceaux.iter().zip([8, 4, 4, 4, 12]).all(|(m, n)| {
            m.len() == n
                && m.chars()
                    .all(|c| c.is_ascii_digit() || ('a'..='f').contains(&c))
        })
}

/// Programme qui lance Notes, lu dans son lanceur `notes.desktop` (premier mot de `Exec`).
pub fn programme(fichier_desktop: &Path) -> Option<String> {
    let texte = fs::read_to_string(fichier_desktop).ok()?;
    let exec = texte
        .lines()
        .take_while(|l| !l.starts_with("[Desktop Action"))
        .find_map(|l| l.strip_prefix("Exec="))?;
    let exec = exec.trim();
    let programme = match exec.strip_prefix('"') {
        Some(reste) => reste.split('"').next()?,
        None => exec.split_whitespace().next()?,
    };
    (!programme.is_empty()).then(|| programme.to_string())
}

/// Argument qui demande à Notes d'ouvrir une page : `--page=<uuid>`.
pub fn argument_page(id: &str) -> Option<String> {
    est_uuid(id).then(|| format!("--page={id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "01a12cf8-f981-789a-bcf1-d2f5b6b17039";

    fn page(dossier: &Path, id: &str, titre: &str) {
        fs::create_dir_all(dossier).unwrap();
        fs::write(
            dossier.join(FICHIER_PAGE),
            format!(
                "---\nid: {id}\ntitle: {titre}\ncreated: 2026-10-11T21:57:56.737Z\n---\nCorps\n"
            ),
        )
        .unwrap();
    }

    #[test]
    fn front_matter_de_notes() {
        assert_eq!(
            lire_meta(&format!("---\nid: {ID}\ntitle: ESGI\nicon: 📚\n---\nx")),
            Some(Meta {
                id: ID.into(),
                titre: "ESGI".into(),
                icone: Some("📚".into())
            })
        );
        assert_eq!(
            lire_meta(&format!(
                "\u{feff}---\r\nid: '{ID}'\r\ntitle: 'L''agent : cours'\r\n---\r\n"
            ))
            .map(|m| (m.id, m.titre)),
            Some((ID.into(), "L'agent : cours".into()))
        );
        assert_eq!(
            lire_meta(&format!(
                "---\nid: \"{ID}\"\ntitle: \"2026 \\\"bis\\\"\" # note\n---\n"
            ))
            .map(|m| (m.id, m.titre)),
            Some((ID.into(), "2026 \"bis\"".into()))
        );
        assert_eq!(lire_meta("---\nid: pas-un-uuid\ntitle: x\n---\n"), None);
        assert_eq!(lire_meta(&format!("---\nid: {ID}\ntitle: \n---\n")), None);
        assert_eq!(lire_meta("# Sans front-matter\n"), None);
        assert_eq!(lire_meta(&format!("---\nid: {ID}\ntitle: x\n")), None);
    }

    #[test]
    fn ordre_de_notes() {
        let coffre = tempfile::tempdir().unwrap();
        let id = |n: u8| format!("01a12cf8-f981-789a-bcf1-d2f5b6b170{n:02}");
        page(&coffre.path().join("P"), &id(1), "Parent");
        page(&coffre.path().join("P/a"), &id(2), "Zèbre");
        page(&coffre.path().join("P/b"), &id(3), "Âne");
        page(&coffre.path().join("P/c"), &id(4), "Moto");
        fs::write(
            coffre.path().join("P/.order"),
            format!("{}\nx\n{}\n", id(4), id(2)),
        )
        .unwrap();
        let titres: Vec<String> = lire_pages(coffre.path())
            .into_iter()
            .map(|p| p.titre)
            .collect();
        assert_eq!(titres, ["Parent", "Moto", "Zèbre", "Âne"]);
    }

    #[test]
    fn icones_comme_notes() {
        let dossier = Path::new("/coffre/Page");
        assert_eq!(
            nature_icone("📚", dossier),
            Some(IconeNote::Emoji("📚".into()))
        );
        assert_eq!(
            nature_icone("🔍️", dossier),
            Some(IconeNote::Emoji("🔍️".into()))
        );
        assert_eq!(
            nature_icone("book-open", dossier),
            Some(IconeNote::Lucide("book-open".into()))
        );
        assert_eq!(
            nature_icone("assets/icone.webp", dossier),
            Some(IconeNote::Image(PathBuf::from(
                "/coffre/Page/assets/icone.webp"
            )))
        );
        for refusee in [
            "assets/../x.png",
            "assets/a/b.png",
            "assets/x.exe",
            "Book Open",
            "a b",
            "",
            "-x",
        ] {
            assert_eq!(nature_icone(refusee, dossier), None, "{refusee}");
        }
    }

    #[test]
    fn pages_et_sous_pages_avec_leur_emplacement() {
        let coffre = tempfile::tempdir().unwrap();
        let id = |n: u8| format!("01a12cf8-f981-789a-bcf1-d2f5b6b170{n:02}");
        page(&coffre.path().join("ESGI"), &id(1), "ESGI");
        page(
            &coffre.path().join("ESGI/Cours"),
            &id(2),
            "Cours d'agentique",
        );
        page(&coffre.path().join("Rangement/Recette"), &id(3), "Tarte");
        page(&coffre.path().join(".trash/Vieille"), &id(4), "Vieille");
        page(&coffre.path().join("ESGI/assets/x"), &id(5), "Image");
        fs::create_dir_all(coffre.path().join("Non importée")).unwrap();
        fs::write(
            coffre.path().join("Non importée/page.md"),
            "Sans front-matter",
        )
        .unwrap();
        let pages = lire_pages(coffre.path());
        let vues: Vec<(&str, &str)> = pages
            .iter()
            .map(|p| (p.titre.as_str(), p.emplacement.as_str()))
            .collect();
        assert_eq!(
            vues,
            [
                ("ESGI", ""),
                ("Cours d'agentique", "ESGI"),
                ("Tarte", "Rangement")
            ]
        );
        assert!(pages[0].a_des_enfants && !pages[1].a_des_enfants);
        assert_eq!(pages[1].parent.as_deref(), Some(id(1).as_str()));
        assert_eq!(pages[2].parent, None);
        assert_eq!(pages[1].id, id(2));
        assert!(pages[1].fichier.ends_with("ESGI/Cours/page.md"));
    }

    #[test]
    fn lanceur_et_argument() {
        let dossier = tempfile::tempdir().unwrap();
        let fichier = dossier.path().join("notes.desktop");
        fs::write(
            &fichier,
            "[Desktop Entry]\nName=Notes\nExec=\"/opt/a b/Notes.AppImage\" %U\n\n[Desktop Action Recherche]\nExec=autre\n",
        )
        .unwrap();
        assert_eq!(
            programme(&fichier).as_deref(),
            Some("/opt/a b/Notes.AppImage")
        );
        fs::write(&fichier, "[Desktop Entry]\nExec=notes --x\n").unwrap();
        assert_eq!(programme(&fichier).as_deref(), Some("notes"));
        assert_eq!(argument_page(ID), Some(format!("--page={ID}")));
        assert_eq!(argument_page("--fond"), None);
        assert!(!est_uuid("01A12CF8-F981-789A-BCF1-D2F5B6B17039"));
    }

    /// Lecture du vrai coffre réglé dans Notes : `cargo test coffre_reel -- --ignored --nocapture`.
    #[test]
    #[ignore = "lit le coffre de Notes du poste"]
    fn coffre_reel() {
        let coffre = coffre().expect("coffre de Notes réglé");
        let pages = lire_pages(&coffre);
        println!(
            "{} pages lues, {} avec un emplacement",
            pages.len(),
            pages.iter().filter(|p| !p.emplacement.is_empty()).count()
        );
        assert!(!pages.is_empty());
    }
}
