//! Skills de Claude Code d'Eloi (`~/.claude/skills/*/SKILL.md`) : une page HTML qui les liste, ouverte
//! par la ligne « Skills Claude » de la section Logiciels. Le contenu de ces fichiers n'est pas de
//! confiance (règle 4) : il passe par du JSON, dont `<` est échappé, et la page ne l'affiche que
//! comme du texte. La ligne est un lanceur `.desktop` écrit dans le cache de la barre : le
//! lancement reste celui des applications (`gio launch`, règle 1).

use super::applications::Application;
use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};

const GABARIT: &str = include_str!("skills.html");
const ICONE: &str = include_str!("skills.svg");
const IDENTIFIANT: &str = "skills-claude.desktop";

/// Un skill, tel que la page le montre.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Skill {
    /// Nom du skill (celui de la commande).
    pub n: String,
    /// Description complète.
    pub d: String,
    /// Indication d'argument (`argument-hint`), vide sans elle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub a: Option<String>,
    /// Lancé seulement par une commande (`disable-model-invocation`), Claude ne le déclenche pas seul.
    pub m: bool,
}

/// Valeurs du front-matter (entre les deux lignes `---`) : `clé: valeur` sur une ligne, ou valeur
/// repliée (`>`, `|`) sur les lignes indentées qui suivent.
fn front_matter(contenu: &str) -> Vec<(String, String)> {
    let mut lignes = contenu.lines();
    if lignes.next().map(str::trim) != Some("---") {
        return Vec::new();
    }
    let mut paires: Vec<(String, String)> = Vec::new();
    for ligne in lignes {
        if ligne.trim() == "---" {
            break;
        }
        let indentee = ligne.starts_with(' ') || ligne.starts_with('\t');
        if indentee {
            if let Some((_, valeur)) = paires.last_mut() {
                if !valeur.is_empty() {
                    valeur.push(' ');
                }
                valeur.push_str(ligne.trim());
            }
            continue;
        }
        if let Some((cle, valeur)) = ligne.split_once(':') {
            let valeur = valeur.trim();
            let valeur = if matches!(valeur, ">" | "|" | ">-" | "|-") {
                ""
            } else {
                valeur
            };
            paires.push((cle.trim().to_string(), valeur.to_string()));
        }
    }
    for (_, valeur) in &mut paires {
        let sans_guillemets = valeur
            .strip_prefix('"')
            .and_then(|v| v.strip_suffix('"'))
            .or_else(|| valeur.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')));
        if let Some(v) = sans_guillemets {
            *valeur = v.to_string();
        }
    }
    paires
}

/// Skill d'un fichier `SKILL.md` ; le nom du dossier sert quand le front-matter n'en donne pas.
fn lire_skill(dossier_nom: &str, contenu: &str) -> Skill {
    let paires = front_matter(contenu);
    let valeur = |cle: &str| {
        paires
            .iter()
            .find(|(c, _)| c == cle)
            .map(|(_, v)| v.clone())
            .filter(|v| !v.is_empty())
    };
    Skill {
        n: valeur("name").unwrap_or_else(|| dossier_nom.to_string()),
        d: valeur("description").unwrap_or_default(),
        a: valeur("argument-hint"),
        m: valeur("disable-model-invocation").as_deref() == Some("true"),
    }
}

/// Skills du dossier (un sous-dossier par skill, avec son `SKILL.md`), par nom.
pub fn lire(dossier: &Path) -> Vec<Skill> {
    let Ok(entrees) = fs::read_dir(dossier) else {
        return Vec::new();
    };
    let mut skills: Vec<Skill> = entrees
        .flatten()
        .filter(|e| !e.file_name().to_string_lossy().starts_with('.'))
        .filter_map(|e| {
            let contenu = fs::read_to_string(e.path().join("SKILL.md")).ok()?;
            Some(lire_skill(&e.file_name().to_string_lossy(), &contenu))
        })
        .collect();
    skills.sort_by_key(|s| s.n.to_lowercase());
    skills
}

/// Page complète : le gabarit avec les skills en JSON. `<` est échappé pour qu'aucun texte ne
/// puisse fermer la balise `script`.
pub fn page(skills: &[Skill]) -> String {
    let json = serde_json::to_string(skills)
        .unwrap_or_else(|_| "[]".into())
        .replace('<', "\\u003c");
    GABARIT.replace("__DONNEES__", &json)
}

fn ecrire_si_change(chemin: &Path, contenu: &str) -> Option<()> {
    if fs::read_to_string(chemin).ok().as_deref() == Some(contenu) {
        return Some(());
    }
    fs::write(chemin, contenu).ok()
}

/// Écrit la page, l'icône et le lanceur dans `sortie`, et rend la ligne de la section Logiciels.
/// Rien sans skill : la ligne n'apparaît pas sur un poste qui n'en a pas.
pub fn preparer(skills_dossier: &Path, sortie: &Path) -> Option<Application> {
    let skills = lire(skills_dossier);
    if skills.is_empty() {
        return None;
    }
    fs::create_dir_all(sortie).ok()?;
    let page_chemin = sortie.join("skills.html");
    let icone_chemin = sortie.join("skills.svg");
    let lanceur = sortie.join(IDENTIFIANT);
    ecrire_si_change(&page_chemin, &page(&skills))?;
    ecrire_si_change(&icone_chemin, ICONE)?;
    // Chemin entre guillemets (`Exec=` : espaces, `"`, `\` et `$` échappés) ; `%` doublé.
    let echappe = page_chemin
        .display()
        .to_string()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('$', "\\$")
        .replace('`', "\\`")
        .replace('%', "%%");
    let contenu = format!(
        "[Desktop Entry]\nType=Application\nName=Skills Claude\nExec=firefox \"{echappe}\"\n"
    );
    ecrire_si_change(&lanceur, &contenu)?;
    Some(Application {
        id: IDENTIFIANT.into(),
        nom: "Skills Claude".into(),
        icone: Some(icone_chemin.display().to_string()),
        mots_cles: ["skill", "skills", "claude code", "commande", "slash"]
            .map(String::from)
            .to_vec(),
        description: Some(format!(
            "Mes skills Claude Code ({}) : chercher, lire, copier la commande.",
            skills.len()
        )),
        fichier: lanceur,
        lancable: true,
        mien: true,
    })
}

/// Dossier des skills de Claude Code et dossier où écrire la page : `~/.claude/skills` et
/// `~/.cache/BarreRecherche/skills`.
pub fn dossiers() -> Option<(PathBuf, PathBuf)> {
    let maison = PathBuf::from(std::env::var_os("HOME")?);
    let cache = std::env::var_os("XDG_CACHE_HOME")
        .map(PathBuf::from)
        .filter(|p| p.is_absolute())
        .unwrap_or_else(|| maison.join(".cache"));
    Some((
        maison.join(".claude").join("skills"),
        cache.join("BarreRecherche").join("skills"),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn front_matter_simple_et_replie() {
        let contenu = "---\nname: audit-code\ndescription: >\n  Premiere ligne\n  suite.\nargument-hint: \"[chemin]\"\ndisable-model-invocation: true\n---\ncorps";
        let skill = lire_skill("dossier", contenu);
        assert_eq!(skill.n, "audit-code");
        assert_eq!(skill.d, "Premiere ligne suite.");
        assert_eq!(skill.a.as_deref(), Some("[chemin]"));
        assert!(skill.m);
    }

    #[test]
    fn sans_front_matter_le_dossier_donne_le_nom() {
        let skill = lire_skill("mon-skill", "juste du texte");
        assert_eq!(skill.n, "mon-skill");
        assert!(skill.d.is_empty());
        assert!(!skill.m);
    }

    #[test]
    fn lire_trie_et_ignore_ce_qui_n_est_pas_un_skill() {
        let tmp = tempfile::tempdir().unwrap();
        for (dossier, nom) in [("b", "Beta"), ("a", "alpha")] {
            fs::create_dir(tmp.path().join(dossier)).unwrap();
            fs::write(
                tmp.path().join(dossier).join("SKILL.md"),
                format!("---\nname: {nom}\n---\n"),
            )
            .unwrap();
        }
        fs::create_dir(tmp.path().join("synced")).unwrap();
        fs::create_dir(tmp.path().join(".cache")).unwrap();
        fs::write(tmp.path().join("fichier.md"), "x").unwrap();
        let noms: Vec<String> = lire(tmp.path()).into_iter().map(|s| s.n).collect();
        assert_eq!(noms, ["alpha", "Beta"]);
    }

    #[test]
    fn la_page_ne_laisse_pas_fermer_le_script() {
        let skill = Skill {
            n: "x".into(),
            d: "</script><img src=x onerror=alert(1)>".into(),
            a: None,
            m: false,
        };
        let html = page(&[skill]);
        let donnees = html.split("id=\"donnees\">").nth(1).unwrap();
        let json = donnees.split("</script>").next().unwrap();
        assert!(json.contains("\\u003c/script>"));
        assert!(!json.contains("</script"));
    }

    #[test]
    fn preparer_ecrit_la_page_et_le_lanceur() {
        let tmp = tempfile::tempdir().unwrap();
        let skills = tmp.path().join("skills");
        fs::create_dir_all(skills.join("un")).unwrap();
        fs::write(skills.join("un").join("SKILL.md"), "---\nname: un\n---\n").unwrap();
        let sortie = tmp.path().join("cache");
        let app = preparer(&skills, &sortie).unwrap();
        assert!(app.mien && app.lancable);
        assert!(sortie.join("skills.html").is_file());
        let lanceur = fs::read_to_string(&app.fichier).unwrap();
        assert!(lanceur.contains("Exec=firefox \""));
        assert!(preparer(&tmp.path().join("absent"), &sortie).is_none());
    }
}
