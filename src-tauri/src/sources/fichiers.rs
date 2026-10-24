//! Fichiers et dossiers du dossier personnel (spécification §3), dans un arbre compact : chaque nœud garde
//! son parent, son nom et son nom normalisé, pas son chemin entier. Construit par un parcours au
//! démarrage, tenu à jour dossier par dossier (`resynchroniser`, appelé sur les avis inotify).
//! La barre ne fait que lire (règle 2).

use crate::floue;
use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Dossiers jamais parcourus, en plus des dossiers cachés : ce qui est généré automatiquement.
const DOSSIERS_EXCLUS: &[&str] = &[
    "node_modules",
    "dist",
    "build",
    "target",
    "venv",
    "__pycache__",
    "coverage",
    "vendor",
];
/// Chemins exclus, relatifs au dossier personnel.
const CHEMINS_EXCLUS: &[&str] = &["go/pkg"];
/// Fichiers qui signalent un dossier généré, quel que soit son nom : cache (spécification
/// CACHEDIR.TAG, posé par cargo dans `target/`) ou environnement Python.
const MARQUES_DE_DOSSIER_GENERE: &[&str] = &["CACHEDIR.TAG", "pyvenv.cfg"];

const SANS_PARENT: u32 = u32::MAX;

struct Noeud {
    parent: u32,
    nom: Box<str>,
    normalise: Box<str>,
    dossier: bool,
    vivant: bool,
}

/// Fichier ou dossier trouvé par la recherche.
#[derive(Debug, Clone, PartialEq)]
pub struct Trouve {
    pub chemin: PathBuf,
    pub nom: String,
    pub dossier: bool,
    pub score: f64,
}

pub struct IndexFichiers {
    racine: PathBuf,
    /// Dossiers absolus jamais parcourus : le coffre de Notes, dont les pages sont cherchées à part.
    exclus: Vec<PathBuf>,
    noeuds: Vec<Noeud>,
    enfants: HashMap<u32, Vec<u32>>,
    /// Nœuds retirés, gardés en place jusqu'à la prochaine reconstruction.
    morts: usize,
}

fn est_exclu(nom: &str, chemin: &Path, racine: &Path, exclus: &[PathBuf]) -> bool {
    if nom.starts_with('.') || DOSSIERS_EXCLUS.contains(&nom) || exclus.iter().any(|e| e == chemin)
    {
        return true;
    }
    if let Ok(relatif) = chemin.strip_prefix(racine) {
        if CHEMINS_EXCLUS
            .iter()
            .any(|exclu| relatif == Path::new(exclu))
        {
            return true;
        }
    }
    MARQUES_DE_DOSSIER_GENERE
        .iter()
        .any(|marque| chemin.join(marque).exists())
}

impl IndexFichiers {
    /// Parcourt tout `racine` (dossier personnel), sans suivre les liens vers des dossiers.
    #[cfg(test)]
    pub fn construire(racine: &Path) -> Self {
        Self::construire_sans(racine, Vec::new())
    }

    /// Comme `construire`, sans les dossiers `exclus` (chemins absolus).
    pub fn construire_sans(racine: &Path, exclus: Vec<PathBuf>) -> Self {
        let mut index = IndexFichiers {
            racine: racine.to_path_buf(),
            exclus,
            noeuds: vec![Noeud {
                parent: SANS_PARENT,
                nom: "".into(),
                normalise: "".into(),
                dossier: true,
                vivant: true,
            }],
            enfants: HashMap::new(),
            morts: 0,
        };
        index.remplir(0, racine);
        index
    }

    /// Nombre d'éléments indexés (le dossier personnel lui-même non compris).
    #[cfg(test)]
    pub fn taille(&self) -> usize {
        self.noeuds.len() - 1 - self.morts
    }

    /// Part de nœuds retirés : au-delà d'un seuil, mieux vaut reconstruire.
    pub fn part_de_morts(&self) -> f64 {
        self.morts as f64 / self.noeuds.len() as f64
    }

    /// Dossiers indexés, à surveiller.
    pub fn dossiers(&self) -> Vec<PathBuf> {
        (0..self.noeuds.len() as u32)
            .filter(|&i| self.noeuds[i as usize].vivant && self.noeuds[i as usize].dossier)
            .map(|i| self.chemin(i))
            .collect()
    }

    fn ajouter(&mut self, parent: u32, nom: &str, dossier: bool) -> u32 {
        let indice = self.noeuds.len() as u32;
        self.noeuds.push(Noeud {
            parent,
            nom: nom.into(),
            normalise: floue::normaliser_en_chaine(nom).into(),
            dossier,
            vivant: true,
        });
        self.enfants.entry(parent).or_default().push(indice);
        indice
    }

    /// Ajoute le contenu de `chemin` sous le nœud `parent`, récursivement.
    fn remplir(&mut self, parent: u32, chemin: &Path) {
        let Ok(entrees) = fs::read_dir(chemin) else {
            return;
        };
        let mut sous_dossiers = Vec::new();
        for entree in entrees.flatten() {
            let Some(nom) = entree.file_name().to_str().map(String::from) else {
                continue;
            };
            let Ok(type_) = entree.file_type() else {
                continue;
            };
            let dossier = type_.is_dir();
            let complet = entree.path();
            if dossier && est_exclu(&nom, &complet, &self.racine, &self.exclus) {
                continue;
            }
            if !dossier && nom.starts_with('.') {
                continue;
            }
            let indice = self.ajouter(parent, &nom, dossier);
            if dossier {
                sous_dossiers.push((indice, complet));
            }
        }
        for (indice, complet) in sous_dossiers {
            self.remplir(indice, &complet);
        }
    }

    pub fn chemin(&self, mut indice: u32) -> PathBuf {
        let mut noms = Vec::new();
        while indice != 0 && indice != SANS_PARENT {
            let noeud = &self.noeuds[indice as usize];
            noms.push(&*noeud.nom);
            indice = noeud.parent;
        }
        let mut chemin = self.racine.clone();
        for nom in noms.into_iter().rev() {
            chemin.push(nom);
        }
        chemin
    }

    /// Nœud vivant de ce chemin, s'il est indexé.
    fn trouver(&self, chemin: &Path) -> Option<u32> {
        let relatif = chemin.strip_prefix(&self.racine).ok()?;
        let mut indice = 0u32;
        for composant in relatif.components() {
            let Component::Normal(nom) = composant else {
                return None;
            };
            let nom = nom.to_str()?;
            indice = *self.enfants.get(&indice)?.iter().find(|&&enfant| {
                let noeud = &self.noeuds[enfant as usize];
                noeud.vivant && &*noeud.nom == nom
            })?;
        }
        Some(indice)
    }

    #[cfg(test)]
    pub fn contient(&self, chemin: &Path) -> bool {
        self.trouver(chemin).is_some_and(|i| i != 0)
    }

    pub fn est_dossier(&self, chemin: &Path) -> Option<bool> {
        self.trouver(chemin)
            .map(|i| self.noeuds[i as usize].dossier)
    }

    fn retirer(&mut self, indice: u32) {
        let mut pile = vec![indice];
        while let Some(i) = pile.pop() {
            let noeud = &mut self.noeuds[i as usize];
            if noeud.vivant {
                noeud.vivant = false;
                self.morts += 1;
            }
            if let Some(enfants) = self.enfants.remove(&i) {
                pile.extend(enfants);
            }
        }
    }

    /// Remet `dossier` d'accord avec le disque : enfants disparus retirés, nouveaux ajoutés (et
    /// parcourus s'ils sont des dossiers). Rend les dossiers ajoutés, à surveiller à leur tour.
    pub fn resynchroniser(&mut self, dossier: &Path) -> Vec<PathBuf> {
        let Some(indice) = self.trouver(dossier) else {
            return Vec::new();
        };
        if !self.noeuds[indice as usize].dossier {
            return Vec::new();
        }
        let mut presents: HashMap<String, bool> = HashMap::new();
        if let Ok(entrees) = fs::read_dir(dossier) {
            for entree in entrees.flatten() {
                let (Some(nom), Ok(type_)) = (
                    entree.file_name().to_str().map(String::from),
                    entree.file_type(),
                ) else {
                    continue;
                };
                let est_dossier = type_.is_dir();
                let exclu = if est_dossier {
                    est_exclu(&nom, &entree.path(), &self.racine, &self.exclus)
                } else {
                    nom.starts_with('.')
                };
                if !exclu {
                    presents.insert(nom, est_dossier);
                }
            }
        }
        let anciens = self.enfants.get(&indice).cloned().unwrap_or_default();
        let mut deja = HashMap::new();
        for enfant in anciens {
            let noeud = &self.noeuds[enfant as usize];
            if !noeud.vivant {
                continue;
            }
            match presents.get(&*noeud.nom) {
                Some(&est_dossier) if est_dossier == noeud.dossier => {
                    deja.insert(noeud.nom.to_string(), ());
                }
                _ => self.retirer(enfant),
            }
        }
        if let Some(liste) = self.enfants.get_mut(&indice) {
            let noeuds = &self.noeuds;
            liste.retain(|&e| noeuds[e as usize].vivant);
        }
        let mut ajoutes = Vec::new();
        for (nom, est_dossier) in presents {
            if deja.contains_key(&nom) {
                continue;
            }
            let nouveau = self.ajouter(indice, &nom, est_dossier);
            if est_dossier {
                let chemin = dossier.join(&nom);
                let premier = self.noeuds.len();
                self.remplir(nouveau, &chemin);
                ajoutes.push(chemin);
                ajoutes.extend(
                    (premier..self.noeuds.len())
                        .filter(|&i| self.noeuds[i].dossier)
                        .map(|i| self.chemin(i as u32)),
                );
            }
        }
        ajoutes
    }

    /// Fichiers et dossiers dont le nom correspond, les `limite` meilleurs, cherchés sur plusieurs
    /// fils. Même classement que `floue::classer` : score, puis nom le plus court.
    pub fn chercher(&self, requete: &[char], limite: usize) -> Vec<Trouve> {
        if requete.is_empty() || limite == 0 {
            return Vec::new();
        }
        let fils = std::thread::available_parallelism()
            .map_or(4, |n| n.get())
            .min(8);
        let morceau = self.noeuds.len().div_ceil(fils).max(1);
        let mut candidats: Vec<(f64, usize, u32)> = std::thread::scope(|portee| {
            let taches: Vec<_> = self
                .noeuds
                .chunks(morceau)
                .enumerate()
                .map(|(rang, noeuds)| {
                    portee.spawn(move || {
                        let mut calculateur = floue::Calculateur::default();
                        let debut = rang * morceau;
                        let mut trouves = Vec::new();
                        for (decalage, noeud) in noeuds.iter().enumerate() {
                            let indice = debut + decalage;
                            if indice == 0
                                || !noeud.vivant
                                || !floue::contient_dans_l_ordre(requete, &noeud.normalise)
                            {
                                continue;
                            }
                            if let Some(score) = calculateur.score(requete, &noeud.normalise) {
                                trouves.push((score, noeud.nom.chars().count(), indice as u32));
                            }
                        }
                        // Garder un peu plus que la limite par fil suffit pour le classement final.
                        meilleurs(&mut trouves, limite);
                        trouves
                    })
                })
                .collect();
            taches
                .into_iter()
                .flat_map(|t| t.join().unwrap_or_default())
                .collect()
        });
        meilleurs(&mut candidats, limite);
        candidats
            .into_iter()
            .map(|(score, _, indice)| {
                let noeud = &self.noeuds[indice as usize];
                Trouve {
                    chemin: self.chemin(indice),
                    nom: noeud.nom.to_string(),
                    dossier: noeud.dossier,
                    score,
                }
            })
            .collect()
    }
}

/// Trie par score décroissant, puis nom le plus court, puis ordre dans l'index, et garde `limite`.
fn meilleurs(trouves: &mut Vec<(f64, usize, u32)>, limite: usize) {
    if trouves.len() > limite {
        trouves.select_nth_unstable_by(limite - 1, comparer);
        trouves.truncate(limite);
    }
    trouves.sort_unstable_by(comparer);
}

fn comparer(a: &(f64, usize, u32), b: &(f64, usize, u32)) -> std::cmp::Ordering {
    b.0.total_cmp(&a.0).then(a.1.cmp(&b.1)).then(a.2.cmp(&b.2))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn arbre() -> tempfile::TempDir {
        let racine = tempfile::tempdir().unwrap();
        let r = racine.path();
        for dossier in [
            "Documents/Rapports",
            "Documents/projet/node_modules/paquet",
            "Documents/projet/target/debug",
            "Documents/projet/src",
            "Documents/cache-perso",
            "Documents/env-python/lib",
            ".config/app",
            "go/pkg/mod",
            "go/src",
        ] {
            fs::create_dir_all(r.join(dossier)).unwrap();
        }
        for fichier in [
            "Documents/Rapports/rapport annuel.pdf",
            "Documents/Rapports/Rapport été.odt",
            "Documents/projet/node_modules/paquet/index.js",
            "Documents/projet/target/debug/rapport.d",
            "Documents/projet/src/main.rs",
            "Documents/cache-perso/CACHEDIR.TAG",
            "Documents/env-python/pyvenv.cfg",
            "Documents/.secret",
            ".config/app/rapport.toml",
            "go/pkg/mod/rapport.go",
            "go/src/main.go",
        ] {
            fs::write(r.join(fichier), "x").unwrap();
        }
        racine
    }

    fn noms(trouves: &[Trouve]) -> Vec<&str> {
        trouves.iter().map(|t| t.nom.as_str()).collect()
    }

    #[test]
    fn indexe_sans_les_dossiers_caches_ni_generes() {
        let racine = arbre();
        let index = IndexFichiers::construire(racine.path());
        let requete = floue::preparer_saisie("rapport");
        let trouves = index.chercher(&requete, 50);
        assert_eq!(
            noms(&trouves),
            ["Rapports", "Rapport été.odt", "rapport annuel.pdf"]
        );
        assert!(trouves[0].dossier);
        assert_eq!(
            trouves[1].chemin,
            racine.path().join("Documents/Rapports/Rapport été.odt")
        );
        for absent in ["index", "pyvenv", "CACHEDIR", "secret", "mod"] {
            assert!(
                index
                    .chercher(&floue::preparer_saisie(absent), 50)
                    .is_empty(),
                "{absent}"
            );
        }
        assert_eq!(
            noms(&index.chercher(&floue::preparer_saisie("main.go"), 50)),
            ["main.go"]
        );
        assert!(index.contient(&racine.path().join("Documents/projet/src/main.rs")));
        assert!(!index.contient(&racine.path().join("Documents/projet/target")));
        assert!(!index.contient(racine.path()));
    }

    #[test]
    fn resynchronise_un_dossier() {
        let racine = arbre();
        let r = racine.path();
        let mut index = IndexFichiers::construire(r);
        let avant = index.taille();

        fs::write(r.join("Documents/Rapports/nouveau bilan.txt"), "x").unwrap();
        fs::remove_file(r.join("Documents/Rapports/rapport annuel.pdf")).unwrap();
        fs::create_dir_all(r.join("Documents/Rapports/2026/T1")).unwrap();
        fs::write(r.join("Documents/Rapports/2026/T1/bilan T1.txt"), "x").unwrap();
        let ajoutes = index.resynchroniser(&r.join("Documents/Rapports"));

        assert_eq!(
            ajoutes,
            [
                r.join("Documents/Rapports/2026"),
                r.join("Documents/Rapports/2026/T1")
            ]
        );
        let bilans = index.chercher(&floue::preparer_saisie("bilan"), 50);
        assert_eq!(noms(&bilans), ["bilan T1.txt", "nouveau bilan.txt"]);
        assert!(!index.contient(&r.join("Documents/Rapports/rapport annuel.pdf")));
        assert_eq!(index.taille(), avant + 3);

        fs::remove_dir_all(r.join("Documents/Rapports/2026")).unwrap();
        index.resynchroniser(&r.join("Documents/Rapports"));
        assert!(index
            .chercher(&floue::preparer_saisie("bilant1"), 50)
            .is_empty());
        assert_eq!(index.taille(), avant);
        assert!(index.part_de_morts() > 0.0);
    }

    #[test]
    fn limite_et_classement() {
        let racine = tempfile::tempdir().unwrap();
        for i in 0..120 {
            fs::write(racine.path().join(format!("note {i:03}.md")), "x").unwrap();
        }
        let index = IndexFichiers::construire(racine.path());
        let trouves = index.chercher(&floue::preparer_saisie("note"), 50);
        assert_eq!(trouves.len(), 50);
        assert!(trouves.windows(2).all(|p| p[0].score >= p[1].score));
    }

    /// Mesure sur le vrai dossier personnel : `cargo test --release -- --ignored mesure_dossier_personnel --nocapture`.
    #[test]
    #[ignore]
    fn mesure_dossier_personnel() {
        let maison = PathBuf::from(std::env::var("HOME").unwrap());
        let debut = std::time::Instant::now();
        let index = IndexFichiers::construire(&maison);
        println!(
            "{} éléments indexés en {:?}",
            index.taille(),
            debut.elapsed()
        );
        for saisie in [
            "r",
            "ra",
            "rap",
            "rapp",
            "rapport",
            "readme",
            "main.rs",
            "photo bapteme",
        ] {
            let requete = floue::preparer_saisie(saisie);
            let debut = std::time::Instant::now();
            let trouves = index.chercher(&requete, 50);
            println!(
                "« {saisie} » : {:?}, premier : {:?}",
                debut.elapsed(),
                trouves.first().map(|t| &t.nom)
            );
        }
    }
}
