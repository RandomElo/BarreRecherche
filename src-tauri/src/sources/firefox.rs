//! Firefox (spécification §3, J6) : marque-pages, mots-clés et historique, lus dans une copie de
//! `places.sqlite` (Firefox verrouille la sienne) ; icônes des sites dans une copie de
//! `favicons.sqlite`. La barre n'écrit jamais dans le profil (règle 2) : les copies vivent dans
//! `~/.cache/BarreRecherche/firefox/`.

use rusqlite::{Connection, OpenFlags, OptionalExtension};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

/// Pages de l'historique gardées au plus, les plus fréquentées d'abord.
const HISTORIQUE_MAXIMUM: usize = 20_000;
/// Au-delà, l'icône d'un site est ignorée.
const POIDS_ICONE_MAXIMUM: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq)]
pub struct Page {
    pub titre: String,
    /// Toujours en `http://` ou `https://` : rien d'autre ne s'ouvre (pas de `javascript:`).
    pub url: String,
    pub marque_page: bool,
    /// Hôte sans `www.` et mot-clé du marque-page : cherchés comme mots-clés.
    pub mots_cles: Vec<String>,
    /// « Frecency » de Firefox : fréquence et fraîcheur des visites.
    pub frecence: i64,
}

/// Hôte de l'adresse, sans `www.`, si elle est en `http(s)`.
pub fn hote(url: &str) -> Option<&str> {
    let reste = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let hote = reste.split(['/', '?', '#']).next()?;
    let hote = hote.rsplit('@').next()?;
    let hote = hote.split(':').next()?;
    (!hote.is_empty()).then(|| hote.strip_prefix("www.").unwrap_or(hote))
}

/// Dossier du profil utilisé par Firefox, d'après `profiles.ini` : celui de la section
/// `[Install…]` (profil de l'installation), sinon le profil marqué `Default=1`.
pub fn profil(dossier_firefox: &Path) -> Option<PathBuf> {
    let contenu = fs::read_to_string(dossier_firefox.join("profiles.ini")).ok()?;
    let mut section = String::new();
    let mut installation = None;
    let mut par_defaut = None;
    let (mut chemin, mut relatif, mut defaut) = (None::<String>, true, false);
    let mut clore = |section: &str, chemin: &mut Option<String>, relatif: bool, defaut: bool| {
        if section.starts_with("Profile") && defaut {
            if let Some(c) = chemin.take() {
                par_defaut = Some(if relatif {
                    dossier_firefox.join(c)
                } else {
                    PathBuf::from(c)
                });
            }
        }
        *chemin = None;
    };
    for ligne in contenu.lines().map(str::trim) {
        if let Some(nom) = ligne.strip_prefix('[').and_then(|l| l.strip_suffix(']')) {
            clore(&section, &mut chemin, relatif, defaut);
            section = nom.to_string();
            (relatif, defaut) = (true, false);
            continue;
        }
        let Some((cle, valeur)) = ligne.split_once('=') else {
            continue;
        };
        match (section.starts_with("Install"), cle) {
            (true, "Default") if installation.is_none() => {
                installation = Some(dossier_firefox.join(valeur));
            }
            (false, "Path") => chemin = Some(valeur.to_string()),
            (false, "IsRelative") => relatif = valeur != "0",
            (false, "Default") => defaut = valeur == "1",
            _ => {}
        }
    }
    clore(&section, &mut chemin, relatif, defaut);
    installation.or(par_defaut).filter(|p| p.is_dir())
}

/// Dossier des copies.
pub fn dossier_copies() -> Option<PathBuf> {
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".cache")))?;
    Some(cache.join("BarreRecherche").join("firefox"))
}

/// Date de modification de la base et de son journal : si elle n'a pas changé, inutile de relire.
pub fn empreinte(profil: &Path, base: &str) -> Option<(SystemTime, Option<SystemTime>)> {
    let date = |nom: String| {
        fs::metadata(profil.join(nom))
            .and_then(|m| m.modified())
            .ok()
    };
    Some((date(base.to_string())?, date(format!("{base}-wal"))))
}

/// Copie `base` (et son journal `-wal`, où sont les derniers changements) du profil vers `copies`,
/// en remplaçant l'ancienne copie d'un coup (renommage). Rend le chemin de la copie.
pub fn copier(profil: &Path, base: &str, copies: &Path) -> Option<PathBuf> {
    fs::create_dir_all(copies).ok()?;
    let provisoire = copies.join(format!("{base}.copie"));
    fs::copy(profil.join(base), &provisoire).ok()?;
    let wal_source = profil.join(format!("{base}-wal"));
    let wal_provisoire = copies.join(format!("{base}.copie-wal"));
    let a_un_wal = fs::copy(&wal_source, &wal_provisoire).is_ok();
    let finale = copies.join(base);
    let wal_final = copies.join(format!("{base}-wal"));
    let _ = fs::remove_file(copies.join(format!("{base}-shm")));
    if a_un_wal {
        fs::rename(&wal_provisoire, &wal_final).ok()?;
    } else {
        let _ = fs::remove_file(&wal_final);
    }
    fs::rename(&provisoire, &finale).ok()?;
    Some(finale)
}

fn ouvrir_base(chemin: &Path) -> Option<Connection> {
    // Lecture et écriture sur la copie seulement : SQLite doit pouvoir y créer son `-shm`.
    Connection::open_with_flags(
        chemin,
        OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    )
    .ok()
}

/// Marque-pages puis historique, une seule ligne par site pour l'historique (sa page la plus
/// fréquentée, demande d'Eloi), aucune pour un site qui a déjà un marque-page.
pub fn lire_pages(base: &Path) -> Vec<Page> {
    let Some(connexion) = ouvrir_base(base) else {
        return Vec::new();
    };
    let mut pages = Vec::new();
    let mut sites = HashSet::new();
    let marque_pages = connexion.prepare(
        "SELECT COALESCE(NULLIF(b.title, ''), p.title, ''), p.url, p.frecency, k.keyword
         FROM moz_bookmarks b JOIN moz_places p ON b.fk = p.id
         LEFT JOIN moz_keywords k ON k.place_id = p.id
         WHERE b.type = 1",
    );
    if let Ok(mut requete) = marque_pages {
        let lignes = requete.query_map([], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, Option<i64>>(2)?,
                l.get::<_, Option<String>>(3)?,
            ))
        });
        for (titre, url, frecence, mot_cle) in lignes.into_iter().flatten().flatten() {
            if let Some(page) = page(titre, url, frecence, true, mot_cle) {
                sites.insert(hote(&page.url).unwrap_or("").to_string());
                pages.push(page);
            }
        }
    }
    let historique = connexion.prepare(
        "SELECT title, url, frecency FROM moz_places
         WHERE visit_count > 0 AND hidden = 0 AND title IS NOT NULL AND title <> ''
         ORDER BY frecency DESC LIMIT ?1",
    );
    if let Ok(mut requete) = historique {
        let lignes = requete.query_map([HISTORIQUE_MAXIMUM as i64], |l| {
            Ok((
                l.get::<_, String>(0)?,
                l.get::<_, String>(1)?,
                l.get::<_, Option<i64>>(2)?,
            ))
        });
        for (titre, url, frecence) in lignes.into_iter().flatten().flatten() {
            let Some(page) = page(titre, url, frecence, false, None) else {
                continue;
            };
            // Triées par fréquentation : la première page d'un site est la plus visitée.
            if sites.insert(hote(&page.url).unwrap_or("").to_string()) {
                pages.push(page);
            }
        }
    }
    pages
}

fn page(
    titre: String,
    url: String,
    frecence: Option<i64>,
    marque_page: bool,
    mot_cle: Option<String>,
) -> Option<Page> {
    let hote = hote(&url)?.to_string();
    let titre = if titre.trim().is_empty() {
        hote.clone()
    } else {
        titre.trim().to_string()
    };
    let mut mots_cles = vec![hote];
    mots_cles.extend(mot_cle.filter(|m| !m.is_empty()));
    Some(Page {
        titre,
        url,
        marque_page,
        mots_cles,
        frecence: frecence.unwrap_or(0).max(0),
    })
}

/// Icône du site de cette page (octets et type MIME), d'après la copie de `favicons.sqlite` :
/// celle de la page, sinon une autre page du même site ; la plus proche de 32 px.
pub fn icone(base: &Path, url: &str) -> Option<(Vec<u8>, &'static str)> {
    let connexion = ouvrir_base(base)?;
    let requete = "SELECT i.data FROM moz_icons i
         JOIN moz_icons_to_pages ip ON ip.icon_id = i.id
         JOIN moz_pages_w_icons p ON p.id = ip.page_id
         WHERE p.page_url {} AND i.data IS NOT NULL
         ORDER BY ABS(i.width - 32) LIMIT 1";
    let exacte = connexion
        .query_row(&requete.replace("{}", "= ?1"), [url], |l| {
            l.get::<_, Vec<u8>>(0)
        })
        .optional()
        .ok()
        .flatten();
    let octets = match exacte {
        Some(octets) => octets,
        None => {
            let origine = origine(url)?;
            connexion
                .query_row(
                    &requete.replace("{}", "LIKE ?1 ESCAPE '\\'"),
                    [format!("{}/%", echapper_like(&origine))],
                    |l| l.get::<_, Vec<u8>>(0),
                )
                .optional()
                .ok()
                .flatten()?
        }
    };
    if octets.len() > POIDS_ICONE_MAXIMUM {
        return None;
    }
    let mime = type_image(&octets)?;
    Some((octets, mime))
}

/// `https://hote[:port]` de l'adresse.
fn origine(url: &str) -> Option<String> {
    let (schema, reste) = url.split_once("://")?;
    let hote = reste.split(['/', '?', '#']).next()?;
    Some(format!("{schema}://{hote}"))
}

fn echapper_like(texte: &str) -> String {
    texte
        .replace('\\', "\\\\")
        .replace('%', "\\%")
        .replace('_', "\\_")
}

/// Type MIME d'après les premiers octets, pour les formats qu'une page affiche.
pub fn type_image(octets: &[u8]) -> Option<&'static str> {
    if octets.starts_with(b"\x89PNG") {
        Some("image/png")
    } else if octets.starts_with(&[0xFF, 0xD8]) {
        Some("image/jpeg")
    } else if octets.starts_with(b"GIF8") {
        Some("image/gif")
    } else if octets.len() > 12 && &octets[0..4] == b"RIFF" && &octets[8..12] == b"WEBP" {
        Some("image/webp")
    } else if octets.starts_with(&[0, 0, 1, 0]) {
        Some("image/x-icon")
    } else {
        let debut = String::from_utf8_lossy(&octets[..octets.len().min(256)]).to_lowercase();
        (debut.trim_start().starts_with("<svg") || debut.trim_start().starts_with("<?xml"))
            .then_some("image/svg+xml")
    }
}

/// Moteurs dont Firefox ne garde pas l'adresse dans `search.json.mozlz4` (moteurs fournis avec
/// lui), par leur identifiant : `{searchTerms}` est remplacé par la recherche.
const MOTEURS_CONNUS: &[(&str, &str)] = &[
    ("google", "https://www.google.com/search?q={searchTerms}"),
    ("ddg", "https://duckduckgo.com/?q={searchTerms}"),
    ("bing", "https://www.bing.com/search?q={searchTerms}"),
    ("qwant", "https://www.qwant.com/?q={searchTerms}"),
    (
        "startpage",
        "https://www.startpage.com/sp/search?query={searchTerms}",
    ),
    ("ecosia", "https://www.ecosia.org/search?q={searchTerms}"),
];

/// Modèle d'adresse d'un moteur connu par son identifiant (`google`).
pub fn moteur_connu(id: &str) -> Option<&'static str> {
    MOTEURS_CONNUS
        .iter()
        .find(|(nom, _)| *nom == id)
        .map(|(_, modele)| *modele)
}

/// Modèle d'adresse du moteur de recherche par défaut de Firefox, lu dans `search.json.mozlz4`
/// du profil : son adresse si Firefox la garde, sinon celle d'un moteur connu. `None` si le
/// fichier est illisible ou le moteur inconnu (la recherche passe alors par `firefox --search`).
pub fn moteur_par_defaut(profil: &Path) -> Option<String> {
    let octets = fs::read(profil.join("search.json.mozlz4")).ok()?;
    let json: serde_json::Value = serde_json::from_slice(&mozlz4(&octets)?).ok()?;
    let id = json["metaData"]["defaultEngineId"].as_str()?;
    let moteur = json["engines"]
        .as_array()?
        .iter()
        .find(|e| e["id"].as_str() == Some(id))?;
    let adresse = moteur["_urls"].as_array().and_then(|adresses| {
        adresses
            .iter()
            .filter(|a| a["type"].as_str().is_none_or(|t| t == "text/html"))
            .filter_map(|a| a["template"].as_str())
            .find(|modele| modele.contains("{searchTerms}"))
    });
    adresse.or_else(|| moteur_connu(id)).map(String::from)
}

/// Décompresse un fichier `mozlz4` de Firefox : `mozLz40\0`, taille en 4 octets, un bloc LZ4.
fn mozlz4(octets: &[u8]) -> Option<Vec<u8>> {
    let reste = octets.strip_prefix(b"mozLz40\0")?;
    let taille = u32::from_le_bytes(reste.get(..4)?.try_into().ok()?) as usize;
    if taille > 16 * 1024 * 1024 {
        return None;
    }
    let source = &reste[4..];
    // Une longueur de 15 continue dans les octets suivants, tant qu'ils valent 255.
    let longueur = |i: &mut usize, mut l: usize| -> Option<usize> {
        if l == 15 {
            loop {
                let octet = *source.get(*i)?;
                *i += 1;
                l += octet as usize;
                if octet != 255 {
                    break;
                }
            }
        }
        Some(l)
    };
    let mut sortie = Vec::with_capacity(taille);
    let mut i = 0;
    while i < source.len() {
        let jeton = source[i];
        i += 1;
        let litteraux = longueur(&mut i, (jeton >> 4) as usize)?;
        sortie.extend_from_slice(source.get(i..i.checked_add(litteraux)?)?);
        i += litteraux;
        if i >= source.len() {
            break;
        }
        let ecart = u16::from_le_bytes([*source.get(i)?, *source.get(i + 1)?]) as usize;
        i += 2;
        let copie = longueur(&mut i, (jeton & 15) as usize)? + 4;
        if ecart == 0 || ecart > sortie.len() || sortie.len() + copie > taille {
            return None;
        }
        let debut = sortie.len() - ecart;
        for k in 0..copie {
            sortie.push(sortie[debut + k]);
        }
    }
    (sortie.len() == taille).then_some(sortie)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hote_sans_www() {
        assert_eq!(hote("https://www.linkedin.com/feed/"), Some("linkedin.com"));
        assert_eq!(hote("http://a.b:8080/x?y"), Some("a.b"));
        assert_eq!(hote("https://moi@site.fr"), Some("site.fr"));
        assert_eq!(hote("javascript:alert(1)"), None);
        assert_eq!(hote("place:sort=8"), None);
        assert_eq!(hote("file:///home/a.html"), None);
    }

    #[test]
    fn profil_de_l_installation_d_abord() {
        let racine = tempfile::tempdir().unwrap();
        for nom in ["a.default", "b.default-release"] {
            fs::create_dir(racine.path().join(nom)).unwrap();
        }
        let ini = "[Install123]\nDefault=b.default-release\nLocked=1\n\n[Profile1]\nName=default\nIsRelative=1\nPath=a.default\nDefault=1\n\n[Profile0]\nPath=b.default-release\n";
        fs::write(racine.path().join("profiles.ini"), ini).unwrap();
        assert_eq!(
            profil(racine.path()),
            Some(racine.path().join("b.default-release"))
        );
        let ini = "[Profile0]\nPath=b.default-release\n\n[Profile1]\nIsRelative=1\nPath=a.default\nDefault=1\n";
        fs::write(racine.path().join("profiles.ini"), ini).unwrap();
        assert_eq!(profil(racine.path()), Some(racine.path().join("a.default")));
        fs::write(racine.path().join("profiles.ini"), "[General]\n").unwrap();
        assert_eq!(profil(racine.path()), None);
    }

    fn base_d_essai(dossier: &Path) -> PathBuf {
        let chemin = dossier.join("places.sqlite");
        let c = Connection::open(&chemin).unwrap();
        c.execute_batch(
            "CREATE TABLE moz_places (id INTEGER PRIMARY KEY, url TEXT, title TEXT, visit_count INTEGER, hidden INTEGER, frecency INTEGER);
             CREATE TABLE moz_bookmarks (id INTEGER PRIMARY KEY, type INTEGER, fk INTEGER, title TEXT);
             CREATE TABLE moz_keywords (id INTEGER PRIMARY KEY, keyword TEXT, place_id INTEGER);
             INSERT INTO moz_places VALUES
               (1, 'https://www.linkedin.com/feed/', 'Fil d''actualité | LinkedIn', 98, 0, 21000),
               (2, 'https://www.linkedin.com/in/jean', 'Jean Dupont | LinkedIn', 3, 0, 900),
               (7, 'https://doc.rust-lang.org/book/', 'Le livre de Rust', 9, 0, 4000),
               (3, 'https://doc.rust-lang.org/', 'Rust', 5, 0, 500),
               (4, 'javascript:alert(1)', 'Piège', 1, 0, 99999),
               (5, 'https://cache.fr/', 'Caché', 1, 1, 50000),
               (6, 'https://jamais.fr/', 'Jamais visité', 0, 0, 10);
             INSERT INTO moz_bookmarks VALUES (1, 1, 3, 'Doc Rust'), (2, 2, NULL, 'Dossier'), (3, 1, 6, '');
             INSERT INTO moz_keywords VALUES (1, 'rs', 3);",
        )
        .unwrap();
        chemin
    }

    #[test]
    fn marque_pages_puis_une_ligne_par_site() {
        let dossier = tempfile::tempdir().unwrap();
        let pages = lire_pages(&base_d_essai(dossier.path()));
        let titres: Vec<_> = pages
            .iter()
            .map(|p| (p.titre.as_str(), p.marque_page))
            .collect();
        assert_eq!(
            titres,
            [
                ("Doc Rust", true),
                ("Jamais visité", true),
                ("Fil d'actualité | LinkedIn", false),
            ]
        );
        assert_eq!(pages[0].mots_cles, ["doc.rust-lang.org", "rs"]);
        assert_eq!(pages[2].url, "https://www.linkedin.com/feed/");
        assert_eq!(pages[2].frecence, 21000);
    }

    #[test]
    fn copie_avec_son_journal() {
        let profil = tempfile::tempdir().unwrap();
        let copies = tempfile::tempdir().unwrap();
        base_d_essai(profil.path());
        fs::write(profil.path().join("places.sqlite-wal"), b"").unwrap();
        let copie = copier(profil.path(), "places.sqlite", copies.path()).unwrap();
        assert_eq!(copie, copies.path().join("places.sqlite"));
        assert!(copies.path().join("places.sqlite-wal").exists());
        assert_eq!(lire_pages(&copie).len(), 3);

        assert!(empreinte(profil.path(), "places.sqlite").is_some());
        assert!(empreinte(profil.path(), "absente.sqlite").is_none());
    }

    #[test]
    fn icone_de_la_page_ou_du_site() {
        let dossier = tempfile::tempdir().unwrap();
        let chemin = dossier.path().join("favicons.sqlite");
        let c = Connection::open(&chemin).unwrap();
        c.execute_batch(
            "CREATE TABLE moz_icons (id INTEGER PRIMARY KEY, width INTEGER, data BLOB);
             CREATE TABLE moz_pages_w_icons (id INTEGER PRIMARY KEY, page_url TEXT);
             CREATE TABLE moz_icons_to_pages (page_id INTEGER, icon_id INTEGER);
             INSERT INTO moz_icons VALUES (1, 16, X'89504E470001'), (2, 32, X'89504E470002');
             INSERT INTO moz_pages_w_icons VALUES (1, 'https://site.fr/a'), (2, 'https://site.fr/b');
             INSERT INTO moz_icons_to_pages VALUES (1, 1), (1, 2), (2, 1);",
        )
        .unwrap();
        let (octets, mime) = icone(&chemin, "https://site.fr/a").unwrap();
        assert_eq!((octets.last(), mime), (Some(&2u8), "image/png"));
        assert!(icone(&chemin, "https://site.fr/c").is_some());
        assert!(icone(&chemin, "https://autre.fr/").is_none());
        assert!(icone(&chemin, "https://site_fr/x").is_none());
    }

    #[test]
    fn formats_d_image() {
        assert_eq!(type_image(b"\x89PNG..."), Some("image/png"));
        assert_eq!(type_image(&[0, 0, 1, 0, 1]), Some("image/x-icon"));
        assert_eq!(type_image(b"  <svg xmlns=\"\">"), Some("image/svg+xml"));
        assert_eq!(type_image(b"<html>"), None);
    }

    /// Fichier `mozlz4` sans aucune répétition : un seul bloc de littéraux.
    fn en_mozlz4(donnees: &[u8]) -> Vec<u8> {
        let mut fichier = b"mozLz40\0".to_vec();
        fichier.extend_from_slice(&(donnees.len() as u32).to_le_bytes());
        fichier.push(0xF0);
        let mut reste = donnees.len() - 15;
        while reste >= 255 {
            fichier.push(255);
            reste -= 255;
        }
        fichier.push(reste as u8);
        fichier.extend_from_slice(donnees);
        fichier
    }

    #[test]
    fn mozlz4_avec_repetition() {
        let mut fichier = b"mozLz40\0".to_vec();
        fichier.extend_from_slice(&9u32.to_le_bytes());
        fichier.extend_from_slice(&[0x32, b'a', b'b', b'c', 3, 0]);
        assert_eq!(mozlz4(&fichier).as_deref(), Some(&b"abcabcabc"[..]));
        // Écart hors de ce qui est déjà écrit : refusé.
        fichier[16] = 9;
        assert_eq!(mozlz4(&fichier), None);
        assert_eq!(mozlz4(b"pas du mozlz4"), None);
    }

    #[test]
    fn moteur_par_defaut_du_profil() {
        let profil = tempfile::tempdir().unwrap();
        let ecrire = |json: &str| {
            fs::write(
                profil.path().join("search.json.mozlz4"),
                en_mozlz4(json.as_bytes()),
            )
            .unwrap()
        };
        ecrire(
            r#"{"metaData":{"defaultEngineId":"perso"},"engines":[{"id":"google"},{"id":"perso","_urls":[{"type":"application/x-suggestions+json","template":"https://s.exemple/?q={searchTerms}"},{"template":"https://www.ecosia.org/search?q={searchTerms}&addon=firefox"}]}]}"#,
        );
        assert_eq!(
            moteur_par_defaut(profil.path()).as_deref(),
            Some("https://www.ecosia.org/search?q={searchTerms}&addon=firefox")
        );
        ecrire(r#"{"metaData":{"defaultEngineId":"ddg"},"engines":[{"id":"ddg","_urls":[]}]}"#);
        assert_eq!(
            moteur_par_defaut(profil.path()).as_deref(),
            Some("https://duckduckgo.com/?q={searchTerms}")
        );
        ecrire(r#"{"metaData":{},"engines":[]}"#);
        assert_eq!(moteur_par_defaut(profil.path()), None);
    }

    /// Moteur par défaut du vrai profil du poste : `cargo test -- --ignored moteur_du_poste --nocapture`.
    #[test]
    #[ignore]
    fn moteur_du_poste() {
        let dossier = PathBuf::from(std::env::var_os("HOME").unwrap()).join(".mozilla/firefox");
        let modele = moteur_par_defaut(&profil(&dossier).unwrap());
        println!("{modele:?}");
        assert!(modele.is_some());
    }
}
