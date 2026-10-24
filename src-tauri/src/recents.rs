//! Éléments récents (spécification §3) : les huit derniers ouverts par la barre, gardés entre deux
//! lancements dans `~/.config/BarreRecherche/recents.json` (spécification §9).

use std::fs;
use std::path::{Path, PathBuf};

pub const MAXIMUM: usize = 8;

pub struct Recents {
    fichier: Option<PathBuf>,
    ids: Vec<String>,
}

/// Dossier de configuration de la barre : `$XDG_CONFIG_HOME/BarreRecherche`, sinon `~/.config/…`.
pub fn dossier_configuration() -> Option<PathBuf> {
    let config = std::env::var_os("XDG_CONFIG_HOME")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))?;
    Some(config.join("BarreRecherche"))
}

impl Recents {
    /// Lit la liste dans `fichier`. Absent ou illisible : liste vide.
    pub fn charger(fichier: Option<PathBuf>) -> Self {
        let ids = fichier
            .as_deref()
            .and_then(|f| fs::read_to_string(f).ok())
            .and_then(|texte| serde_json::from_str::<Vec<String>>(&texte).ok())
            .unwrap_or_default()
            .into_iter()
            .take(MAXIMUM)
            .collect();
        Recents { fichier, ids }
    }

    pub fn ids(&self) -> &[String] {
        &self.ids
    }

    /// Met `id` en tête, sans doublon, huit au plus, et enregistre.
    pub fn ajouter(&mut self, id: &str) {
        self.ids.retain(|existant| existant != id);
        self.ids.insert(0, id.to_string());
        self.ids.truncate(MAXIMUM);
        if let Some(fichier) = &self.fichier {
            if let Err(erreur) = enregistrer(fichier, &self.ids) {
                eprintln!("Récents non enregistrés dans {fichier:?} : {erreur}");
            }
        }
    }
}

/// Écrit dans un fichier voisin puis le renomme : jamais de fichier à moitié écrit.
fn enregistrer(fichier: &Path, ids: &[String]) -> std::io::Result<()> {
    if let Some(dossier) = fichier.parent() {
        fs::create_dir_all(dossier)?;
    }
    let temporaire = fichier.with_extension("json.nouveau");
    fs::write(&temporaire, serde_json::to_string(ids)?)?;
    fs::rename(temporaire, fichier)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn en_tete_sans_doublon_huit_au_plus_et_garde_entre_deux_lancements() {
        let dossier = tempfile::tempdir().unwrap();
        let fichier = dossier.path().join("config/recents.json");
        let mut recents = Recents::charger(Some(fichier.clone()));
        assert!(recents.ids().is_empty());
        for i in 0..10 {
            recents.ajouter(&format!("app:{i}"));
        }
        recents.ajouter("app:5");
        assert_eq!(recents.ids().len(), MAXIMUM);
        assert_eq!(recents.ids()[0], "app:5");
        assert_eq!(recents.ids()[1], "app:9");
        assert_eq!(recents.ids().iter().filter(|i| *i == "app:5").count(), 1);

        let relus = Recents::charger(Some(fichier));
        assert_eq!(relus.ids(), recents.ids());
    }

    #[test]
    fn fichier_abime_donne_une_liste_vide() {
        let dossier = tempfile::tempdir().unwrap();
        let fichier = dossier.path().join("recents.json");
        fs::write(&fichier, "{pas du json").unwrap();
        assert!(Recents::charger(Some(fichier)).ids().is_empty());
    }
}
