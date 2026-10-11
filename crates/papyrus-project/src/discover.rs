//! Découverte des `.psc` à traiter.
//!
//! Aucune recherche d'installation : seuls les chemins donnés sont parcourus.
//!
//! # Fragments
//!
//! Le Creation Kit génère un script par fragment de quête, de dialogue, de
//! scène ou de perk. Personne ne les écrit à la main, et les reformater ou les
//! linter n'apporte que du bruit : ils sont exclus par défaut.
//!
//! Mesuré sur les trois jeux, un fragment se reconnaît à l'un de deux signes,
//! sans faux positif :
//!
//! - il est rangé sous un dossier `Fragments` (Fallout 4, Starfield) ;
//! - son nom contient un segment `TIF`, `QF`, `SF`, `PF`, `PRKF` ou `TERM` :
//!   `TIF__0010CB55`, `DLC2_PF_…_02020993`, `QF_ccBGSSSE025_DarkRevFrag`.
//!   Skyrim range ses 10 437 fragments à plat, Fallout 4 en laisse 394 hors
//!   des dossiers `Fragments`.

use std::io;
use std::path::{Path, PathBuf};

/// Ce que la découverte doit inclure.
#[derive(Debug, Clone, Copy, Default)]
pub struct DiscoverOptions {
    /// Inclure les fragments générés par le Creation Kit.
    pub include_fragments: bool,
}

/// Les segments de nom qui désignent un fragment.
const FRAGMENT_MARKERS: [&str; 6] = ["TIF", "QF", "SF", "PF", "PRKF", "TERM"];

/// Vrai si `path` désigne un `.psc`.
pub fn is_script(path: &Path) -> bool {
    path.extension()
        .is_some_and(|e| e.eq_ignore_ascii_case("psc"))
}

/// Vrai si le nom de fichier est celui d'un fragment.
pub fn is_fragment_name(path: &Path) -> bool {
    let Some(stem) = path.file_stem().and_then(|s| s.to_str()) else {
        return false;
    };
    // Le dernier segment n'est jamais un marqueur : il faut un `_` après.
    let mut segments: Vec<&str> = stem.split('_').collect();
    segments.pop();
    segments
        .iter()
        .any(|s| FRAGMENT_MARKERS.iter().any(|m| s.eq_ignore_ascii_case(m)))
}

fn is_fragment_dir(path: &Path) -> bool {
    path.file_name()
        .is_some_and(|n| n.eq_ignore_ascii_case("fragments"))
}

/// Les `.psc` désignés par `roots`, triés.
///
/// Un fichier donné explicitement est toujours retenu, fragment ou non : la
/// demande est explicite. Un dossier est parcouru récursivement.
pub fn discover(roots: &[PathBuf], options: DiscoverOptions) -> io::Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for root in roots {
        if root.is_dir() {
            walk(root, options, &mut out)?;
        } else if root.exists() {
            out.push(root.clone());
        } else {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} : introuvable", root.display()),
            ));
        }
    }
    out.sort();
    out.dedup();
    Ok(out)
}

fn walk(dir: &Path, options: DiscoverOptions, out: &mut Vec<PathBuf>) -> io::Result<()> {
    for entry in std::fs::read_dir(dir)? {
        let path = entry?.path();
        if path.is_dir() {
            if options.include_fragments || !is_fragment_dir(&path) {
                walk(&path, options, out)?;
            }
        } else if is_script(&path) && (options.include_fragments || !is_fragment_name(&path)) {
            out.push(path);
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn les_fragments_se_reconnaissent_a_leur_nom() {
        for name in [
            "TIF__0010CB55.psc",
            "QF_MQ101_0002610C.psc",
            "DLC2_PF_DLC2MQ04NelothToWorks_02020993.psc",
            "ccBGSSSE001_TIF__05000C73.psc",
            "QF_ccBGSSSE025_DarkRevFrag.psc",
            "PRKF_VRWorkshopShared_PerkOp_0100003F.psc",
            "tif__0010cb55.psc",
        ] {
            assert!(is_fragment_name(Path::new(name)), "{name}");
        }
    }

    #[test]
    fn les_scripts_ordinaires_ne_sont_pas_des_fragments() {
        for name in [
            "ObjectReference.psc",
            "SQ_ParentScript.psc",
            "SFBGS00DRandomEnableOnLoadScript.psc",
            "MyTIFHelper.psc",
            // Le marqueur doit être suivi d'un segment.
            "Foo_QF.psc",
            "QF.psc",
        ] {
            assert!(!is_fragment_name(Path::new(name)), "{name}");
        }
    }
}
