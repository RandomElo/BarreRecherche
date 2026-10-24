//! Aperçu de la ligne active (spécification §6, J11) : premières lignes d'un texte, image, PDF (envoyé en
//! octets à pdf.js), contenu d'un dossier, description d'une application, adresse d'une page.
//! Seul ce qui est indexé se lit. Le contenu n'est pas de confiance (règle 4) : il part en texte
//! brut ou en adresse `data:` d'image, jamais en HTML.

use crate::cibles::Cibles;
use crate::recherche::{emplacement, Index, Nature};
use base64::Engine;
use serde::Serialize;
use std::fs;
use std::io::Read;
use std::path::Path;

/// Octets lus au plus pour un texte, et lignes gardées.
const TEXTE_OCTETS: usize = 16 * 1024;
const TEXTE_LIGNES: usize = 80;
/// Au-delà, pas d'aperçu : une image trop lourde ralentirait la barre.
const IMAGE_MAXIMUM: u64 = 10 * 1024 * 1024;
pub const PDF_MAXIMUM: u64 = 40 * 1024 * 1024;
/// Éléments d'un dossier montrés au plus.
const DOSSIER_ELEMENTS: usize = 40;

/// En-tête de l'aperçu : nom et détails (emplacement, taille).
#[derive(Debug, Serialize, PartialEq)]
pub struct Entete {
    pub titre: String,
    pub details: String,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ElementDossier {
    pub nom: String,
    pub dossier: bool,
    /// Noms d'icônes du thème selon le type, séparés par des virgules (`theme:` pour `icone`).
    pub icone: String,
    /// Dans l'index de la barre : sinon (dossier caché, généré), l'aperçu le dit.
    pub indexe: bool,
}

#[derive(Debug, Serialize, PartialEq)]
#[serde(tag = "genre", rename_all = "camelCase")]
pub enum Apercu {
    Texte {
        entete: Entete,
        texte: String,
        /// Le fichier continue après ce qui est montré.
        tronque: bool,
    },
    Image {
        entete: Entete,
        adresse: String,
    },
    /// Fichier sans aperçu possible (binaire inconnu, trop lourd) : l'en-tête seul.
    Fichier {
        entete: Entete,
    },
    /// Page HTML rendue dans un cadre isolé (sans scripts ni réseau), servie par `apercu://`.
    Html {
        entete: Entete,
        adresse: String,
    },
    /// Les octets se demandent à part (`apercu_pdf`), pdf.js dessine la première page.
    Pdf {
        entete: Entete,
    },
    Dossier {
        entete: Entete,
        elements: Vec<ElementDossier>,
        total: usize,
    },
    /// Page de Notes : ses sous-pages (comme le contenu d'un dossier), puis son texte.
    Note {
        entete: Entete,
        enfants: Vec<crate::recherche::Resultat>,
        total: usize,
        texte: String,
        tronque: bool,
    },
    Application {
        entete: Entete,
        description: Option<String>,
    },
    Page {
        entete: Entete,
        adresse: String,
    },
}

/// Taille lisible : « 812 octets », « 2 Ko », « 1,4 Mo ».
pub fn taille_lisible(octets: u64) -> String {
    const KO: f64 = 1024.0;
    let n = octets as f64;
    if octets < 1024 {
        format!("{octets} octets")
    } else if n < KO * KO {
        format!("{} Ko", (n / KO).round())
    } else if n < KO * KO * KO {
        format!("{:.1} Mo", n / KO / KO).replace('.', ",")
    } else {
        format!("{:.1} Go", n / KO / KO / KO).replace('.', ",")
    }
}

fn type_image(chemin: &Path) -> Option<&'static str> {
    match chemin.extension()?.to_str()?.to_ascii_lowercase().as_str() {
        "png" => Some("image/png"),
        "jpg" | "jpeg" => Some("image/jpeg"),
        "gif" => Some("image/gif"),
        "webp" => Some("image/webp"),
        "bmp" => Some("image/bmp"),
        "svg" => Some("image/svg+xml"),
        _ => None,
    }
}

fn est_une_page(chemin: &Path) -> bool {
    chemin
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| {
            ["html", "htm", "xhtml"]
                .iter()
                .any(|h| e.eq_ignore_ascii_case(h))
        })
}

/// Adresse `apercu://` d'un fichier : son chemin, chaque octet hors `A-Za-z0-9-._~/` encodé.
pub fn adresse_apercu(chemin: &Path) -> String {
    use std::os::unix::ffi::OsStrExt;
    let mut adresse = String::from("apercu://localhost");
    for &octet in chemin.as_os_str().as_bytes() {
        if octet.is_ascii_alphanumeric() || b"-._~/".contains(&octet) {
            adresse.push(octet as char);
        } else {
            adresse.push_str(&format!("%{octet:02X}"));
        }
    }
    adresse
}

/// Chemin d'une adresse `apercu://` (sa partie chemin, décodée), ou `None` si mal formée.
pub fn chemin_de_l_adresse(chemin_encode: &str) -> Option<std::path::PathBuf> {
    use std::os::unix::ffi::OsStringExt;
    let octets = chemin_encode.as_bytes();
    let mut decode = Vec::with_capacity(octets.len());
    let mut i = 0;
    while i < octets.len() {
        if octets[i] == b'%' {
            let hexa = std::str::from_utf8(octets.get(i + 1..i + 3)?).ok()?;
            decode.push(u8::from_str_radix(hexa, 16).ok()?);
            i += 3;
        } else {
            decode.push(octets[i]);
            i += 1;
        }
    }
    let chemin = std::path::PathBuf::from(std::ffi::OsString::from_vec(decode));
    // Pas de `..` : le chemin demandé doit être celui d'un fichier indexé, tel quel.
    (chemin.is_absolute()
        && !chemin
            .components()
            .any(|c| c == std::path::Component::ParentDir))
    .then_some(chemin)
}

/// Type MIME servi par `apercu://`.
fn type_servi(chemin: &Path) -> &'static str {
    match chemin
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
        .as_str()
    {
        "html" | "htm" => "text/html; charset=utf-8",
        "xhtml" => "application/xhtml+xml",
        "css" => "text/css",
        "woff2" => "font/woff2",
        "woff" => "font/woff",
        "ttf" => "font/ttf",
        "otf" => "font/otf",
        _ => type_image(chemin).unwrap_or("application/octet-stream"),
    }
}

/// Règles imposées à toute page servie par `apercu://` : ni script, ni réseau, ni cadre ; styles
/// et images de la page seulement (règles 3 et 4). Le cadre `sandbox` de la barre coupe aussi les
/// scripts.
pub const CSP_APERCU: &str = "default-src 'none'; style-src apercu: 'unsafe-inline'; img-src apercu: data:; font-src apercu: data:; base-uri 'none'; form-action 'none'";
/// Au-delà, `apercu://` ne sert pas le fichier.
const SERVI_MAXIMUM: u64 = 20 * 1024 * 1024;

/// Réponse de `apercu://` : seulement un fichier indexé (la page, ses styles, ses images).
pub fn servir(index: &Index, chemin_encode: &str) -> tauri::http::Response<Vec<u8>> {
    let refus = |statut: u16| {
        tauri::http::Response::builder()
            .status(statut)
            .header("Content-Security-Policy", CSP_APERCU)
            .body(Vec::new())
            .expect("réponse vide")
    };
    let Some(chemin) = chemin_de_l_adresse(chemin_encode) else {
        return refus(400);
    };
    let Some((chemin, false)) = index.fichier(&format!("fichier:{}", chemin.display())) else {
        return refus(404);
    };
    match fs::metadata(&chemin) {
        Ok(m) if m.len() <= SERVI_MAXIMUM => {}
        _ => return refus(404),
    }
    match fs::read(&chemin) {
        Ok(octets) => tauri::http::Response::builder()
            .header("Content-Type", type_servi(&chemin))
            .header("Content-Security-Policy", CSP_APERCU)
            .header("Access-Control-Allow-Origin", "*")
            .body(octets)
            .unwrap_or_else(|_| refus(500)),
        Err(_) => refus(404),
    }
}

pub fn est_un_pdf(chemin: &Path) -> bool {
    chemin
        .extension()
        .and_then(|e| e.to_str())
        .is_some_and(|e| e.eq_ignore_ascii_case("pdf"))
}

/// Début du fichier en texte, s'il en est un (pas d'octet nul, UTF-8 valide hors dernier
/// caractère coupé) : `TEXTE_LIGNES` lignes au plus, et s'il continue après.
pub fn debut_de_texte(octets: &[u8], fichier_plus_long: bool) -> Option<(String, bool)> {
    if octets.contains(&0) {
        return None;
    }
    let texte = match std::str::from_utf8(octets) {
        Ok(texte) => texte,
        // Un caractère coupé par la limite de lecture n'empêche pas d'être du texte.
        Err(erreur) if erreur.error_len().is_none() => {
            std::str::from_utf8(&octets[..erreur.valid_up_to()]).ok()?
        }
        Err(_) => return None,
    };
    let lignes: Vec<&str> = texte.lines().collect();
    let tronque = fichier_plus_long || lignes.len() > TEXTE_LIGNES;
    Some((lignes[..lignes.len().min(TEXTE_LIGNES)].join("\n"), tronque))
}

/// Aperçu d'un fichier ou d'un dossier : l'en-tête seul quand le contenu ne se montre pas (binaire
/// inconnu, trop lourd), `None` s'il est illisible. `emplacement` : le dossier tel que la liste le
/// montre ; `indexe` dit si un élément d'un dossier est dans l'index.
pub fn de_fichier(
    chemin: &Path,
    dossier: bool,
    emplacement: &str,
    indexe: &dyn Fn(&Path) -> bool,
) -> Option<Apercu> {
    let titre = chemin.file_name()?.to_string_lossy().into_owned();
    if dossier {
        return de_dossier(chemin, titre, emplacement, indexe);
    }
    let taille = fs::metadata(chemin).ok()?.len();
    let entete = Entete {
        titre,
        details: format!("{emplacement} · {}", taille_lisible(taille)),
    };
    Some(contenu_de_fichier(chemin, taille, entete))
}

/// Contenu montrable d'un fichier, sinon son en-tête seul.
fn contenu_de_fichier(chemin: &Path, taille: u64, entete: Entete) -> Apercu {
    if let Some(mime) = type_image(chemin) {
        if taille <= IMAGE_MAXIMUM {
            if let Ok(octets) = fs::read(chemin) {
                let adresse = format!(
                    "data:{mime};base64,{}",
                    base64::engine::general_purpose::STANDARD.encode(octets)
                );
                return Apercu::Image { entete, adresse };
            }
        }
        return Apercu::Fichier { entete };
    }
    if est_un_pdf(chemin) {
        return if taille <= PDF_MAXIMUM {
            Apercu::Pdf { entete }
        } else {
            Apercu::Fichier { entete }
        };
    }
    if est_une_page(chemin) && taille <= SERVI_MAXIMUM {
        return Apercu::Html {
            entete,
            adresse: adresse_apercu(chemin),
        };
    }
    let mut octets = Vec::with_capacity(TEXTE_OCTETS);
    let lu = fs::File::open(chemin)
        .and_then(|f| f.take(TEXTE_OCTETS as u64).read_to_end(&mut octets))
        .is_ok();
    match lu
        .then(|| debut_de_texte(&octets, taille > octets.len() as u64))
        .flatten()
    {
        Some((texte, tronque)) => Apercu::Texte {
            entete,
            texte,
            tronque,
        },
        None => Apercu::Fichier { entete },
    }
}

fn de_dossier(
    chemin: &Path,
    titre: String,
    emplacement: &str,
    indexe: &dyn Fn(&Path) -> bool,
) -> Option<Apercu> {
    let mut elements: Vec<ElementDossier> = fs::read_dir(chemin)
        .ok()?
        .flatten()
        .filter_map(|entree| {
            let nom = entree.file_name().to_string_lossy().into_owned();
            let chemin = entree.path();
            let dossier = chemin.is_dir();
            (!nom.starts_with('.')).then(|| ElementDossier {
                icone: crate::icones::noms_selon_type(&chemin, dossier).join(","),
                indexe: indexe(&chemin),
                dossier,
                nom,
            })
        })
        .collect();
    let total = elements.len();
    // Dossiers d'abord, puis par nom sans tenir compte de la casse.
    elements.sort_by(|a, b| {
        b.dossier
            .cmp(&a.dossier)
            .then_with(|| a.nom.to_lowercase().cmp(&b.nom.to_lowercase()))
    });
    elements.truncate(DOSSIER_ELEMENTS);
    let compte = match total {
        0 => "vide".to_string(),
        1 => "1 élément".to_string(),
        n => format!("{n} éléments"),
    };
    Some(Apercu::Dossier {
        entete: Entete {
            titre,
            details: format!("{emplacement} · {compte}"),
        },
        elements,
        total,
    })
}

/// Aperçu de l'élément de cet identifiant (seulement ce qui est indexé), ou `None`.
#[tauri::command]
pub async fn apercu(index: tauri::State<'_, Index>, id: String) -> Result<Option<Apercu>, ()> {
    if let Some((chemin, dossier)) = index.fichier(&id) {
        let emplacement = emplacement(chemin.parent().unwrap_or(&chemin));
        let indexe = |p: &Path| index.fichier(&format!("fichier:{}", p.display())).is_some();
        return Ok(de_fichier(&chemin, dossier, &emplacement, &indexe));
    }
    let Some(element) = index.element(&id) else {
        return Ok(None);
    };
    Ok(Some(match element.nature {
        Nature::Application(application) => Apercu::Application {
            entete: Entete {
                titre: application.nom,
                details: "Application".into(),
            },
            description: application.description,
        },
        Nature::Page(page) => Apercu::Page {
            entete: Entete {
                titre: page.titre,
                details: if page.marque_page {
                    "Marque-page de Firefox".into()
                } else {
                    "Historique de Firefox".into()
                },
            },
            adresse: page.url,
        },
        // Page de Notes : ses sous-pages, puis son texte (markdown) sans le front-matter, en
        // texte brut.
        Nature::Note(page) => {
            let lu = std::fs::read_to_string(&page.fichier).unwrap_or_default();
            let corps = crate::sources::notes::decouper(&lu).map_or(lu.as_str(), |(_, c)| c);
            let debut: String = corps.chars().take(TEXTE_OCTETS).collect();
            let (texte, tronque) =
                debut_de_texte(debut.as_bytes(), debut.len() < corps.len()).unwrap_or_default();
            let mut sous_pages = index.sous_pages(&page.id, "", Cibles::default());
            let total = sous_pages.len();
            sous_pages.truncate(DOSSIER_ELEMENTS);
            Apercu::Note {
                entete: Entete {
                    titre: page.titre,
                    details: if page.emplacement.is_empty() {
                        "Page de Notes".into()
                    } else {
                        format!("Notes · {}", page.emplacement)
                    },
                },
                enfants: sous_pages,
                total,
                texte: texte.trim_start_matches('\n').to_string(),
                tronque,
            }
        }
    }))
}

/// Octets d'un PDF indexé, pour pdf.js.
#[tauri::command]
pub async fn apercu_pdf(
    index: tauri::State<'_, Index>,
    id: String,
) -> Result<tauri::ipc::Response, String> {
    let (chemin, _) = index
        .fichier(&id)
        .filter(|(chemin, dossier)| !dossier && est_un_pdf(chemin))
        .ok_or("Pas un PDF indexé.")?;
    if fs::metadata(&chemin).map_err(|e| e.to_string())?.len() > PDF_MAXIMUM {
        return Err("PDF trop lourd pour l'aperçu.".into());
    }
    Ok(tauri::ipc::Response::new(
        fs::read(&chemin).map_err(|e| e.to_string())?,
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adresse_apercu_aller_retour() {
        let chemin = Path::new("/home/a/Mes pages/l'été #1.html");
        let adresse = adresse_apercu(chemin);
        assert_eq!(
            adresse,
            "apercu://localhost/home/a/Mes%20pages/l%27%C3%A9t%C3%A9%20%231.html"
        );
        let partie = adresse.strip_prefix("apercu://localhost").unwrap();
        assert_eq!(chemin_de_l_adresse(partie).as_deref(), Some(chemin));
        assert_eq!(chemin_de_l_adresse("/a/../etc/passwd"), None);
        assert_eq!(chemin_de_l_adresse("/a/%2E%2E/etc"), None);
        assert_eq!(chemin_de_l_adresse("relatif"), None);
        assert_eq!(chemin_de_l_adresse("/a/%zz"), None);
    }

    #[test]
    fn regles_des_pages_servies() {
        assert!(!CSP_APERCU.contains("script"));
        assert!(CSP_APERCU.starts_with("default-src 'none'"));
        assert_eq!(type_servi(Path::new("/a/b.CSS")), "text/css");
        assert_eq!(
            type_servi(Path::new("/a/b.exe")),
            "application/octet-stream"
        );
    }

    #[test]
    fn tailles() {
        assert_eq!(taille_lisible(812), "812 octets");
        assert_eq!(taille_lisible(2048), "2 Ko");
        assert_eq!(taille_lisible(1_468_006), "1,4 Mo");
    }

    #[test]
    fn texte_ou_pas() {
        assert_eq!(
            debut_de_texte(b"a\nb", false),
            Some(("a\nb".to_string(), false))
        );
        assert_eq!(debut_de_texte(b"a\0b", false), None);
        assert_eq!(debut_de_texte(&[0xff, 0xfe, 0x41], false), None);
        // « é » coupé en fin de lecture : encore du texte.
        assert_eq!(
            debut_de_texte(&[b'a', 0xc3], true),
            Some(("a".to_string(), true))
        );
        let long = "x\n".repeat(200);
        let (texte, tronque) = debut_de_texte(long.as_bytes(), false).unwrap();
        assert_eq!((texte.lines().count(), tronque), (TEXTE_LIGNES, true));
    }

    #[test]
    fn apercus_de_fichiers() {
        let racine = tempfile::tempdir().unwrap();
        let r = racine.path();
        fs::write(r.join("notes.md"), "# Titre\n<script>alert(1)</script>").unwrap();
        fs::write(r.join("image.PNG"), b"\x89PNG").unwrap();
        fs::write(r.join("doc.pdf"), b"%PDF-1.7").unwrap();
        fs::write(r.join("binaire.bin"), [0u8, 1, 2]).unwrap();
        fs::write(r.join("page.html"), "<p>x</p>").unwrap();
        fs::create_dir(r.join("Projets")).unwrap();
        fs::write(r.join(".cache"), "").unwrap();
        let tout = |_: &Path| true;
        let sauf_projets = |p: &Path| !p.ends_with("Projets");

        match de_fichier(&r.join("notes.md"), false, "~/x", &tout).unwrap() {
            Apercu::Texte { entete, texte, .. } => {
                assert_eq!(entete.titre, "notes.md");
                assert!(entete.details.starts_with("~/x · "));
                // Le texte part tel quel : la page l'affiche en texte, sans l'interpréter.
                assert!(texte.contains("<script>"));
            }
            autre => panic!("{autre:?}"),
        }
        assert!(matches!(
            de_fichier(&r.join("image.PNG"), false, "~", &tout),
            Some(Apercu::Image { adresse, .. }) if adresse.starts_with("data:image/png;base64,")
        ));
        assert!(matches!(
            de_fichier(&r.join("doc.pdf"), false, "~", &tout),
            Some(Apercu::Pdf { .. })
        ));
        assert!(matches!(
            de_fichier(&r.join("binaire.bin"), false, "~", &tout),
            Some(Apercu::Fichier { .. })
        ));
        assert!(matches!(
            de_fichier(&r.join("page.html"), false, "~", &tout),
            Some(Apercu::Html { adresse, .. }) if adresse.starts_with("apercu://localhost/")
        ));
        match de_fichier(r, true, "~", &sauf_projets).unwrap() {
            Apercu::Dossier {
                elements, total, ..
            } => {
                assert_eq!(total, 6);
                assert_eq!(elements[0].nom, "Projets");
                assert!(elements[0].dossier && !elements[0].indexe);
                assert!(elements[0].icone.ends_with("folder"));
                assert!(elements[1..]
                    .iter()
                    .all(|e| e.indexe && !e.icone.is_empty()));
                assert!(elements.iter().all(|e| !e.nom.starts_with('.')));
            }
            autre => panic!("{autre:?}"),
        }
    }

    /// Les noms des champs sont ceux qu'attend `src/lib/systeme.ts` (un champ en deux mots y
    /// casserait l'aperçu : le renommage ne vaut que pour les variantes).
    #[test]
    fn apercu_d_une_page_de_notes_tel_que_la_page_le_lit() {
        let json = serde_json::to_value(Apercu::Note {
            entete: Entete {
                titre: "Cours".into(),
                details: "Page de Notes".into(),
            },
            enfants: Vec::new(),
            total: 0,
            texte: "x".into(),
            tronque: false,
        })
        .unwrap();
        let mut cles: Vec<&String> = json.as_object().unwrap().keys().collect();
        cles.sort();
        assert_eq!(
            cles,
            ["enfants", "entete", "genre", "texte", "total", "tronque"]
        );
        assert_eq!(json["genre"], "note");
    }
}
