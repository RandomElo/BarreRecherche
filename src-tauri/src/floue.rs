//! Recherche floue (spécification §3), reprise de `Notes/packages/core/src/search/fuzzy.ts` : les lettres
//! tapées doivent apparaître dans le titre, dans l'ordre, pas forcément côte à côte. Accents, casse
//! et espaces de la saisie sont ignorés. Les lettres qui se suivent et celles qui ouvrent un mot
//! comptent davantage. Programmation dynamique en O(lettres tapées × longueur du titre).

use unicode_normalization::char::is_combining_mark;
use unicode_normalization::UnicodeNormalization;

const POINTS_PAR_LETTRE: f64 = 1.0;
const BONUS_DEBUT_DE_MOT: f64 = 8.0;
const BONUS_DEBUT_DE_TITRE: f64 = 4.0;
const BONUS_LETTRES_QUI_SE_SUIVENT: f64 = 6.0;
const PENALITE_PAR_LETTRE_SAUTEE: f64 = 0.5;

/// Correspondance trouvée : score, et position (en caractères du titre d'origine) de chaque lettre trouvée.
#[derive(Debug, Clone, PartialEq)]
pub struct Correspondance {
    pub score: f64,
    pub positions: Vec<usize>,
}

/// Texte comparable : sans accents, en minuscules, ligatures défaites. Chaque caractère obtenu
/// garde la position (en caractères) de celui d'où il vient.
pub fn normaliser(texte: &str) -> (Vec<char>, Vec<usize>) {
    let mut resultat = Vec::with_capacity(texte.len());
    let mut origines = Vec::with_capacity(texte.len());
    for (position, caractere) in texte.chars().enumerate() {
        for minuscule in caractere.to_lowercase() {
            let ligature = match minuscule {
                'œ' => Some("oe"),
                'æ' => Some("ae"),
                'ß' => Some("ss"),
                _ => None,
            };
            match ligature {
                Some(lettres) => {
                    for lettre in lettres.chars() {
                        resultat.push(lettre);
                        origines.push(position);
                    }
                }
                None => {
                    for simple in std::iter::once(minuscule)
                        .nfd()
                        .filter(|c| !is_combining_mark(*c))
                    {
                        resultat.push(simple);
                        origines.push(position);
                    }
                }
            }
        }
    }
    (resultat, origines)
}

/// Saisie prête à chercher : normalisée, sans espaces.
pub fn preparer_saisie(saisie: &str) -> Vec<char> {
    normaliser(saisie)
        .0
        .into_iter()
        .filter(|c| !c.is_whitespace())
        .collect()
}

/// Meilleure façon de retrouver la saisie (déjà préparée) dans le titre, ou `None` si une de ses
/// lettres manque. Une saisie vide correspond à tout, avec un score nul.
pub fn correspondance(requete: &[char], titre: &str) -> Option<Correspondance> {
    if requete.is_empty() {
        return Some(Correspondance {
            score: 0.0,
            positions: Vec::new(),
        });
    }
    let (texte, origines) = normaliser(titre);
    let (n, m) = (texte.len(), requete.len());
    if m > n {
        return None;
    }

    let valeur_de_la_lettre = |i: usize| -> f64 {
        let mut valeur = POINTS_PAR_LETTRE;
        if i == 0 {
            valeur += BONUS_DEBUT_DE_MOT + BONUS_DEBUT_DE_TITRE;
        } else if !texte[i - 1].is_alphanumeric() && texte[i].is_alphanumeric() {
            valeur += BONUS_DEBUT_DE_MOT;
        }
        valeur
    };

    // meilleur[j][i] : meilleur score quand la lettre j de la saisie est prise à la position i du titre.
    // venant[j][i] : position de la lettre j - 1 dans ce meilleur cas, pour retrouver le chemin.
    let mut meilleur: Vec<Vec<f64>> = Vec::with_capacity(m);
    let mut venant: Vec<Vec<Option<usize>>> = Vec::with_capacity(m);
    for (j, lettre) in requete.iter().enumerate() {
        let mut ligne = vec![f64::NEG_INFINITY; n];
        let mut origine = vec![None; n];
        let precedente = if j > 0 { Some(&meilleur[j - 1]) } else { None };
        // Meilleur départ parmi les positions déjà passées, pénalité des lettres sautées comprise :
        // max sur k de (precedente[k] + pénalité × k), corrigé ensuite de pénalité × (i - 1).
        let mut max_ecarte = f64::NEG_INFINITY;
        let mut max_ecarte_en = None;
        for i in 0..n {
            if let Some(precedente) = precedente {
                if i >= 2 {
                    let candidat = precedente[i - 2] + PENALITE_PAR_LETTRE_SAUTEE * (i - 2) as f64;
                    if candidat > max_ecarte {
                        max_ecarte = candidat;
                        max_ecarte_en = Some(i - 2);
                    }
                }
            }
            if texte[i] != *lettre {
                continue;
            }
            let Some(precedente) = precedente else {
                ligne[i] = valeur_de_la_lettre(i);
                continue;
            };
            let collee = if i >= 1 {
                precedente[i - 1] + BONUS_LETTRES_QUI_SE_SUIVENT
            } else {
                f64::NEG_INFINITY
            };
            let ecartee = if i >= 1 {
                max_ecarte - PENALITE_PAR_LETTRE_SAUTEE * (i - 1) as f64
            } else {
                f64::NEG_INFINITY
            };
            if collee == f64::NEG_INFINITY && ecartee == f64::NEG_INFINITY {
                continue;
            }
            if collee >= ecartee {
                ligne[i] = collee + valeur_de_la_lettre(i);
                origine[i] = Some(i - 1);
            } else {
                ligne[i] = ecartee + valeur_de_la_lettre(i);
                origine[i] = max_ecarte_en;
            }
        }
        meilleur.push(ligne);
        venant.push(origine);
    }

    let mut fin = None;
    let mut score_final = f64::NEG_INFINITY;
    for (i, score) in meilleur[m - 1].iter().enumerate() {
        if *score > score_final {
            score_final = *score;
            fin = Some(i);
        }
    }
    let mut i = fin?;

    let mut positions_normalisees = Vec::with_capacity(m);
    for j in (0..m).rev() {
        positions_normalisees.push(i);
        match venant[j][i] {
            Some(avant) => i = avant,
            None => break,
        }
    }
    positions_normalisees.reverse();
    // Une ligature (« œ » pour « oe ») donne deux caractères comparables pour un seul affiché.
    let mut positions: Vec<usize> = positions_normalisees
        .into_iter()
        .map(|i| origines[i])
        .collect();
    positions.dedup();
    Some(Correspondance {
        score: score_final,
        positions,
    })
}

/// Lettres de la saisie présentes dans le texte, dans l'ordre : test rapide, avant le calcul du score.
pub fn contient_dans_l_ordre(requete: &[char], texte: &str) -> bool {
    let mut attendues = requete.iter();
    let Some(mut attendue) = attendues.next() else {
        return true;
    };
    for caractere in texte.chars() {
        if caractere == *attendue {
            match attendues.next() {
                Some(suivante) => attendue = suivante,
                None => return true,
            }
        }
    }
    false
}

/// Score seul, sans les positions, sur un texte déjà normalisé (`normaliser`), avec des tampons
/// réutilisés d'un appel à l'autre : pour classer des centaines de milliers de noms de fichiers.
/// Donne le même score que `correspondance`.
#[derive(Default)]
pub struct Calculateur {
    texte: Vec<char>,
    precedente: Vec<f64>,
    ligne: Vec<f64>,
}

impl Calculateur {
    pub fn score(&mut self, requete: &[char], texte_normalise: &str) -> Option<f64> {
        if requete.is_empty() {
            return Some(0.0);
        }
        self.texte.clear();
        self.texte.extend(texte_normalise.chars());
        let texte = &self.texte;
        let (n, m) = (texte.len(), requete.len());
        if m > n {
            return None;
        }
        let valeur_de_la_lettre = |i: usize| -> f64 {
            let mut valeur = POINTS_PAR_LETTRE;
            if i == 0 {
                valeur += BONUS_DEBUT_DE_MOT + BONUS_DEBUT_DE_TITRE;
            } else if !texte[i - 1].is_alphanumeric() && texte[i].is_alphanumeric() {
                valeur += BONUS_DEBUT_DE_MOT;
            }
            valeur
        };
        self.precedente.clear();
        self.precedente.resize(n, f64::NEG_INFINITY);
        self.ligne.clear();
        self.ligne.resize(n, f64::NEG_INFINITY);
        for (j, lettre) in requete.iter().enumerate() {
            let mut max_ecarte = f64::NEG_INFINITY;
            // `i` lit aussi les cases i - 1 et i - 2 : une boucle d'indices est plus claire ici.
            #[allow(clippy::needless_range_loop)]
            for i in 0..n {
                if j > 0 && i >= 2 {
                    let candidat =
                        self.precedente[i - 2] + PENALITE_PAR_LETTRE_SAUTEE * (i - 2) as f64;
                    if candidat > max_ecarte {
                        max_ecarte = candidat;
                    }
                }
                self.ligne[i] = f64::NEG_INFINITY;
                if texte[i] != *lettre {
                    continue;
                }
                if j == 0 {
                    self.ligne[i] = valeur_de_la_lettre(i);
                    continue;
                }
                let collee = if i >= 1 {
                    self.precedente[i - 1] + BONUS_LETTRES_QUI_SE_SUIVENT
                } else {
                    f64::NEG_INFINITY
                };
                let ecartee = if i >= 1 {
                    max_ecarte - PENALITE_PAR_LETTRE_SAUTEE * (i - 1) as f64
                } else {
                    f64::NEG_INFINITY
                };
                let meilleure = collee.max(ecartee);
                if meilleure != f64::NEG_INFINITY {
                    self.ligne[i] = meilleure + valeur_de_la_lettre(i);
                }
            }
            std::mem::swap(&mut self.precedente, &mut self.ligne);
        }
        let meilleur = self
            .precedente
            .iter()
            .copied()
            .fold(f64::NEG_INFINITY, f64::max);
        (meilleur != f64::NEG_INFINITY).then_some(meilleur)
    }
}

/// Texte normalisé en chaîne, pour le garder dans un index.
pub fn normaliser_en_chaine(texte: &str) -> String {
    normaliser(texte).0.into_iter().collect()
}

/// Part du score gardée quand la saisie est trouvée dans un mot-clé plutôt que dans le titre :
/// le titre passe devant à qualité égale.
const PART_D_UN_MOT_CLE: f64 = 0.5;

/// Éléments qui correspondent à la saisie, le meilleur en premier, `limite` au plus. À score égal,
/// le titre le plus court passe devant, puis l'ordre d'origine. Sans mots-clés : sert aux tests repris de Notes.
#[cfg(test)]
pub fn classer<'a, T>(
    saisie: &str,
    elements: &'a [T],
    titre_de: impl Fn(&T) -> &str,
    limite: usize,
) -> Vec<(&'a T, Correspondance)> {
    classer_avec_mots_cles(saisie, elements, titre_de, |_| &[], limite)
}

/// Comme `classer`, en cherchant aussi dans les mots-clés de chaque élément. Trouvé seulement
/// dans un mot-clé, l'élément garde une part du score et aucune lettre du titre n'est surlignée.
pub fn classer_avec_mots_cles<'a, T>(
    saisie: &str,
    elements: &'a [T],
    titre_de: impl Fn(&T) -> &str,
    mots_cles_de: impl Fn(&T) -> &[String],
    limite: usize,
) -> Vec<(&'a T, Correspondance)> {
    let requete = preparer_saisie(saisie);
    let mut resultats: Vec<(usize, &'a T, Correspondance)> = elements
        .iter()
        .enumerate()
        .filter_map(|(rang, element)| {
            let par_titre = correspondance(&requete, titre_de(element));
            let par_mot_cle = mots_cles_de(element)
                .iter()
                .filter_map(|mot| correspondance(&requete, mot))
                .map(|c| c.score * PART_D_UN_MOT_CLE)
                .max_by(f64::total_cmp);
            let meilleure = match (par_titre, par_mot_cle) {
                (Some(titre), Some(mot)) if mot > titre.score => Correspondance {
                    score: mot,
                    positions: Vec::new(),
                },
                (Some(titre), _) => titre,
                (None, Some(mot)) => Correspondance {
                    score: mot,
                    positions: Vec::new(),
                },
                (None, None) => return None,
            };
            Some((rang, element, meilleure))
        })
        .collect();
    resultats.sort_by(|a, b| {
        b.2.score
            .total_cmp(&a.2.score)
            .then_with(|| {
                titre_de(a.1)
                    .chars()
                    .count()
                    .cmp(&titre_de(b.1).chars().count())
            })
            .then_with(|| a.0.cmp(&b.0))
    });
    resultats.truncate(limite);
    resultats.into_iter().map(|(_, e, c)| (e, c)).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const TITRES: [&str; 6] = [
        "Courses",
        "Refonte du NAS",
        "Recettes de cuisine",
        "Réunion équipe",
        "Cœur du sujet",
        "Notes de course à pied",
    ];

    fn trouver(saisie: &str, titre: &str) -> Option<Correspondance> {
        correspondance(&preparer_saisie(saisie), titre)
    }

    fn classer_titres(saisie: &str) -> Vec<&'static str> {
        classer(saisie, &TITRES, |t| t, 50)
            .into_iter()
            .map(|(t, _)| *t)
            .collect()
    }

    #[test]
    fn retire_accents_et_majuscules_en_gardant_les_positions() {
        let (texte, origines) = normaliser("Été à Noël");
        assert_eq!(texte.iter().collect::<String>(), "ete a noel");
        assert_eq!(origines, (0..10).collect::<Vec<_>>());
    }

    #[test]
    fn defait_les_ligatures() {
        let (texte, origines) = normaliser("Cœur");
        assert_eq!(texte.iter().collect::<String>(), "coeur");
        assert_eq!(origines, [0, 1, 1, 2, 3]);
    }

    #[test]
    fn trouve_les_lettres_dans_l_ordre_meme_separees() {
        assert_eq!(
            trouver("rfnas", "Refonte du NAS").unwrap().positions,
            [0, 2, 11, 12, 13]
        );
    }

    #[test]
    fn refuse_une_lettre_manquante_ou_dans_le_desordre() {
        assert!(trouver("xyz", "Courses").is_none());
        assert!(trouver("cs", "Courses").is_some());
        assert!(trouver("sc", "Courses").is_none());
    }

    #[test]
    fn ignore_accents_casse_et_espaces_de_la_saisie() {
        assert!(trouver("REUNION EQUIPE", "Réunion équipe").is_some());
        assert!(trouver("reunionequipe", "Réunion équipe").is_some());
        assert_eq!(
            trouver("coeur", "Cœur du sujet").unwrap().positions,
            [0, 1, 2, 3]
        );
    }

    #[test]
    fn saisie_vide_correspond_a_tout() {
        assert_eq!(
            trouver("  ", "Courses"),
            Some(Correspondance {
                score: 0.0,
                positions: vec![]
            })
        );
    }

    #[test]
    fn prefere_lettres_qui_se_suivent_et_debuts_de_mot() {
        let collees = trouver("cour", "Courses").unwrap();
        let eparses = trouver("cour", "Cuisine pour tous").unwrap();
        assert!(collees.score > eparses.score);
        assert_eq!(
            trouver("nas", "Refonte du NAS").unwrap().positions,
            [11, 12, 13]
        );
    }

    #[test]
    fn positions_en_caracteres_avec_accents_avant() {
        assert_eq!(
            trouver("equ", "Réunion équipe").unwrap().positions,
            [8, 9, 10]
        );
    }

    #[test]
    fn met_le_meilleur_titre_en_premier() {
        assert_eq!(classer_titres("course")[0], "Courses");
        assert!(classer_titres("course").contains(&"Notes de course à pied"));
        assert_eq!(classer_titres("rec")[0], "Recettes de cuisine");
    }

    #[test]
    fn ne_garde_que_les_titres_qui_correspondent() {
        assert_eq!(classer_titres("nas"), ["Refonte du NAS"]);
        assert!(classer_titres("qwerty").is_empty());
    }

    #[test]
    fn calculateur_donne_le_meme_score() {
        let mut calculateur = Calculateur::default();
        for titre in TITRES.iter().chain(["Cuisine pour tous", "a", ""].iter()) {
            let normalise = normaliser_en_chaine(titre);
            for saisie in [
                "course", "rfnas", "cour", "nas", "coeur", "e", "sc", "xyz", "eqp",
            ] {
                let requete = preparer_saisie(saisie);
                assert_eq!(
                    calculateur.score(&requete, &normalise),
                    correspondance(&requete, titre).map(|c| c.score),
                    "{saisie} dans {titre}"
                );
                assert_eq!(
                    contient_dans_l_ordre(&requete, &normalise),
                    correspondance(&requete, titre).is_some(),
                    "{saisie} dans {titre}"
                );
            }
        }
    }

    #[test]
    fn trouve_par_mot_cle_apres_le_titre() {
        let elements = [
            ("Firefox", vec!["Navigateur Web".to_string()]),
            ("Navigation GPS", vec![]),
            ("Terminal", vec![]),
        ];
        let resultats = classer_avec_mots_cles("navig", &elements, |e| e.0, |e| e.1.as_slice(), 50);
        let titres: Vec<_> = resultats.iter().map(|(e, _)| e.0).collect();
        assert_eq!(titres, ["Navigation GPS", "Firefox"]);
        assert!(resultats[1].1.positions.is_empty());
    }

    #[test]
    fn a_score_egal_garde_l_ordre_donne_et_respecte_la_limite() {
        let elements = ["b", "a", "c"];
        let ordre: Vec<_> = classer("", &elements, |t| t, 50)
            .into_iter()
            .map(|(t, _)| *t)
            .collect();
        assert_eq!(ordre, ["b", "a", "c"]);
        assert_eq!(classer("", &elements, |t| t, 2).len(), 2);
    }
}
