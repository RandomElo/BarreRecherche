// Les commandes de l'application sont déclarées ici : chacune n'est appelable que par
// les fenêtres dont la capacité (capabilities/*.json) donne sa permission « allow-… ».
const COMMANDES: &[&str] = &[
    "ajuster_taille",
    "cacher_barre",
    "ouvrir_reglages",
    "etat_reglages",
    "definir_demarrage_auto",
    "quitter",
    "chercher",
    "ouvrir",
    "chercher_sur_le_web",
    "ouvrir_site",
    "icone",
    "contexte_ouverture",
    "relayer_ctrl_k_ctrl_o",
    "apercu",
    "apercu_pdf",
    "bourse_chercher",
    "bourse_apercu",
];

/// Variables du `.env` du projet que la barre reçoit à la construction (`@bourse`, spécification §9).
const VARIABLES_ENV: [&str; 3] = ["BOURSE_ADRESSE", "BOURSE_ADRESSE_SECOURS", "BOURSE_CLE"];

/// Lit `../.env` (hors dépôt, `.gitignore`) et passe ses variables `@bourse` au compilateur :
/// l'AppImage installée ne lit pas le dépôt. Format : `NOM=valeur` par ligne, `export` devant
/// permis, guillemets autour de la valeur retirés, lignes vides et `#` ignorées.
fn variables_du_env() {
    let fichier = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../.env");
    println!("cargo:rerun-if-changed={}", fichier.display());
    let Ok(texte) = std::fs::read_to_string(&fichier) else {
        return;
    };
    for ligne in texte.lines().map(str::trim) {
        let ligne = ligne.strip_prefix("export ").unwrap_or(ligne);
        let Some((nom, valeur)) = ligne.split_once('=') else {
            continue;
        };
        let (nom, valeur) = (nom.trim(), valeur.trim());
        let valeur = ['"', '\'']
            .iter()
            .find_map(|g| valeur.strip_prefix(*g)?.strip_suffix(*g))
            .unwrap_or(valeur);
        if VARIABLES_ENV.contains(&nom) && !valeur.contains(['\n', '\r']) {
            println!("cargo:rustc-env={nom}={valeur}");
        }
    }
}

fn main() {
    variables_du_env();
    tauri_build::try_build(
        tauri_build::Attributes::new()
            .app_manifest(tauri_build::AppManifest::new().commands(COMMANDES)),
    )
    .expect("échec de la préparation de Tauri");
}
