//! Garde-fous sur la configuration Tauri : CSP stricte, rien vers le réseau, permissions minimales (règles 3 et 4).

use serde_json::Value;
use std::fs;
use std::path::Path;

fn lire_json(chemin: &str) -> Value {
    let complet = Path::new(env!("CARGO_MANIFEST_DIR")).join(chemin);
    let texte =
        fs::read_to_string(&complet).unwrap_or_else(|e| panic!("lecture de {complet:?} : {e}"));
    serde_json::from_str(&texte).unwrap_or_else(|e| panic!("JSON invalide dans {complet:?} : {e}"))
}

fn directives(csp: &str) -> Vec<(String, Vec<String>)> {
    csp.split(';')
        .map(str::trim)
        .filter(|d| !d.is_empty())
        .map(|d| {
            let mut morceaux = d.split_whitespace().map(String::from);
            let nom = morceaux.next().unwrap_or_default();
            (nom, morceaux.collect())
        })
        .collect()
}

fn sources<'a>(liste: &'a [(String, Vec<String>)], nom: &str) -> &'a [String] {
    liste
        .iter()
        .find(|(n, _)| n == nom)
        .map(|(_, s)| s.as_slice())
        .unwrap_or_else(|| panic!("directive {nom} absente de la CSP"))
}

#[test]
fn csp_de_production_stricte() {
    let config = lire_json("tauri.conf.json");
    let csp = config["app"]["security"]["csp"]
        .as_str()
        .expect("CSP absente");
    let liste = directives(csp);

    assert_eq!(sources(&liste, "default-src"), ["'self'"]);
    assert_eq!(sources(&liste, "script-src"), ["'self'"]);
    assert_eq!(sources(&liste, "object-src"), ["'none'"]);
    // Seul cadre permis : l'aperçu des pages HTML, servi par la barre (`apercu.rs`).
    assert_eq!(sources(&liste, "frame-src"), ["apercu:"]);
    for (nom, valeurs) in &liste {
        for valeur in valeurs {
            assert!(
                !valeur.contains("unsafe"),
                "{nom} autorise {valeur} en production"
            );
        }
    }
    // Pas de réseau : seul le canal IPC de Tauri est permis.
    for valeur in sources(&liste, "connect-src") {
        assert!(
            valeur == "ipc:" || valeur == "http://ipc.localhost",
            "connect-src autorise {valeur}"
        );
    }
}

#[test]
fn csp_de_developpement_sans_reseau_exterieur() {
    let config = lire_json("tauri.conf.json");
    let csp = config["app"]["security"]["devCsp"]
        .as_str()
        .expect("devCsp absente");
    for (nom, valeurs) in directives(csp) {
        for valeur in valeurs {
            assert!(
                !valeur.contains("unsafe-eval"),
                "{nom} autorise unsafe-eval"
            );
            if valeur.contains("://") {
                assert!(
                    valeur.contains("127.0.0.1") || valeur == "http://ipc.localhost",
                    "{nom} ouvre vers l'extérieur : {valeur}"
                );
            }
        }
    }
}

#[test]
fn api_tauri_non_exposee_globalement() {
    let config = lire_json("tauri.conf.json");
    assert_eq!(config["app"]["withGlobalTauri"], Value::Bool(false));
}

/// Permissions de chaque fenêtre, listées une par une. Un jalon qui en ajoute une l'inscrit ici.
const PERMISSIONS: &[(&str, &[&str])] = &[
    (
        "barre",
        &[
            "allow-ajuster-taille",
            "allow-cacher-barre",
            "allow-ouvrir-reglages",
            "allow-etat-reglages",
            "allow-chercher",
            "allow-ouvrir",
            "allow-chercher-sur-le-web",
            "allow-ouvrir-site",
            "allow-icone",
            "allow-contexte-ouverture",
            "allow-relayer-ctrl-k-ctrl-o",
            "allow-apercu",
            "allow-apercu-pdf",
            "allow-bourse-chercher",
            "allow-bourse-apercu",
        ],
    ),
    (
        "reglages",
        &[
            "allow-etat-reglages",
            "allow-definir-demarrage-auto",
            "allow-quitter",
        ],
    ),
];

#[test]
fn permissions_minimales() {
    let dossier = Path::new(env!("CARGO_MANIFEST_DIR")).join("capabilities");
    let mut fichiers: Vec<String> = fs::read_dir(&dossier)
        .expect("dossier capabilities")
        .map(|entree| entree.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    fichiers.sort();
    let mut attendus: Vec<String> = PERMISSIONS
        .iter()
        .map(|(nom, _)| format!("{nom}.json"))
        .collect();
    attendus.sort();
    assert_eq!(fichiers, attendus, "capacités inattendues");

    for (nom, autorisees) in PERMISSIONS {
        let capacite = lire_json(&format!("capabilities/{nom}.json"));
        assert_eq!(capacite["windows"], serde_json::json!([nom]));
        let permissions: Vec<&str> = capacite["permissions"]
            .as_array()
            .expect("permissions absentes")
            .iter()
            .map(|p| {
                p.as_str()
                    .or_else(|| p["identifier"].as_str())
                    .unwrap_or_default()
            })
            .collect();
        assert_eq!(&permissions, autorisees, "permissions de {nom}");
    }
}
