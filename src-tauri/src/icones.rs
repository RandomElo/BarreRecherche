//! Icônes des résultats : le nom donné par une source (thème d'icônes) ou un chemin absolu, résolu
//! en fichier par GTK, puis envoyé à la page en adresse `data:` (la CSP n'autorise que `self` et
//! `data:` pour les images). Une image dans `<img>` n'exécute rien, même en SVG.

use base64::Engine;
use std::fs;
use std::path::{Path, PathBuf};

/// Taille demandée au thème, en pixels (affichée à 20 px, nette en écran à forte densité).
const TAILLE: i32 = 32;
/// Au-delà, le fichier n'est pas une icône raisonnable : ignoré.
const POIDS_MAXIMUM: u64 = 512 * 1024;

/// Type MIME d'après l'extension, pour les formats qu'une page sait afficher.
pub fn type_mime(fichier: &Path) -> Option<&'static str> {
    match fichier.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "svg" => Some("image/svg+xml"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        // Icônes image des pages de Notes.
        "webp" => Some("image/webp"),
        "gif" => Some("image/gif"),
        "avif" => Some("image/avif"),
        _ => None,
    }
}

/// Fichier de l'icône : chemin absolu tel quel, sinon nom cherché dans le thème courant.
/// Appelé sur le fil principal (GTK).
pub fn fichier(icone: &str) -> Option<PathBuf> {
    use gtk::prelude::IconThemeExt;
    let chemin = Path::new(icone);
    if chemin.is_absolute() {
        return chemin.is_file().then(|| chemin.to_path_buf());
    }
    // « firefox.png » se cherche comme « firefox ».
    let nom = match chemin.extension().and_then(|e| e.to_str()) {
        Some("png" | "svg" | "xpm") => chemin.file_stem()?.to_str()?,
        _ => icone,
    };
    let theme = gtk::IconTheme::default()?;
    theme
        .lookup_icon(nom, TAILLE, gtk::IconLookupFlags::FORCE_SIZE)
        .and_then(|info| info.filename())
}

/// Icône du thème pour un fichier selon son type (comme le gestionnaire de fichiers), ou pour un
/// dossier (dossiers spéciaux : Documents, Téléchargements…). Appelé sur le fil principal (GTK).
pub fn fichier_selon_type(chemin: &Path, dossier: bool) -> Option<PathBuf> {
    fichier_parmi(&noms_selon_type(chemin, dossier))
}

/// Noms d'icônes du thème pour ce fichier ou ce dossier, du plus précis au plus général. Sans le
/// thème lui-même : appelable hors du fil principal.
pub fn noms_selon_type(chemin: &Path, dossier: bool) -> Vec<String> {
    use gtk::prelude::*;
    if dossier {
        vec![nom_de_dossier(chemin).to_string(), "folder".to_string()]
    } else {
        let (type_contenu, _) = gtk::gio::content_type_guess(Some(chemin), &[]);
        let icone = gtk::gio::content_type_get_icon(&type_contenu);
        let mut noms: Vec<String> = icone
            .downcast_ref::<gtk::gio::ThemedIcon>()
            .map(|i| i.names().iter().map(|n| n.to_string()).collect())
            .unwrap_or_default();
        noms.push("text-x-generic".to_string());
        noms
    }
}

/// Fichier de la première de ces icônes que le thème connaît. Appelé sur le fil principal (GTK).
pub fn fichier_parmi(noms: &[String]) -> Option<PathBuf> {
    use gtk::prelude::IconThemeExt;
    let theme = gtk::IconTheme::default()?;
    noms.iter().find_map(|nom| {
        theme
            .lookup_icon(nom, TAILLE, gtk::IconLookupFlags::FORCE_SIZE)
            .and_then(|info| info.filename())
    })
}

/// Nom d'icône d'un dossier : spécial (dossier personnel, Documents, Téléchargements…) ou `folder`.
fn nom_de_dossier(chemin: &Path) -> &'static str {
    use gtk::glib::{user_special_dir, UserDirectory};
    if std::env::var_os("HOME").is_some_and(|h| Path::new(&h) == chemin) {
        return "user-home";
    }
    [
        (UserDirectory::Desktop, "user-desktop"),
        (UserDirectory::Documents, "folder-documents"),
        (UserDirectory::Downloads, "folder-download"),
        (UserDirectory::Music, "folder-music"),
        (UserDirectory::Pictures, "folder-pictures"),
        (UserDirectory::Videos, "folder-videos"),
        (UserDirectory::PublicShare, "folder-publicshare"),
        (UserDirectory::Templates, "folder-templates"),
    ]
    .into_iter()
    .find(|(dossier, _)| user_special_dir(*dossier).is_some_and(|d| d == chemin))
    .map_or("folder", |(_, nom)| nom)
}

/// Nom d'icône du thème acceptable venant de la page : lettres, chiffres, `-`, `_`, `.`, `+`
/// (jamais un chemin).
pub fn nom_valide(nom: &str) -> bool {
    !nom.is_empty()
        && nom.len() <= 128
        && nom
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.+".contains(c))
}

/// Adresse `data:` d'une image déjà lue (icône d'un site).
pub fn adresse_octets(octets: &[u8], mime: &str) -> String {
    format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(octets)
    )
}

/// Adresse `data:` du fichier, ou `None` s'il est trop gros, illisible ou d'un format inconnu.
pub fn adresse_data(fichier: &Path) -> Option<String> {
    let mime = type_mime(fichier)?;
    if fs::metadata(fichier).ok()?.len() > POIDS_MAXIMUM {
        return None;
    }
    let octets = fs::read(fichier).ok()?;
    Some(format!(
        "data:{mime};base64,{}",
        base64::engine::general_purpose::STANDARD.encode(octets)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn noms_d_icones_venant_de_la_page() {
        assert!(nom_valide("application-pdf") && nom_valide("x-office-document+x"));
        assert!(!nom_valide("/usr/share/icons/a.png") && !nom_valide("../a") && !nom_valide(""));
    }

    #[test]
    fn formats_affichables() {
        assert_eq!(type_mime(Path::new("/a/b.PNG")), Some("image/png"));
        assert_eq!(type_mime(Path::new("/a/b.svg")), Some("image/svg+xml"));
        assert_eq!(type_mime(Path::new("/a/b.xpm")), None);
        assert_eq!(type_mime(Path::new("/a/b")), None);
    }

    #[test]
    fn adresse_data_d_un_petit_fichier() {
        let dossier = tempfile::tempdir().unwrap();
        let fichier = dossier.path().join("i.png");
        fs::write(&fichier, b"abc").unwrap();
        assert_eq!(
            adresse_data(&fichier).as_deref(),
            Some("data:image/png;base64,YWJj")
        );
        let gros = dossier.path().join("gros.png");
        fs::write(&gros, vec![0u8; (POIDS_MAXIMUM + 1) as usize]).unwrap();
        assert_eq!(adresse_data(&gros), None);
    }
}
