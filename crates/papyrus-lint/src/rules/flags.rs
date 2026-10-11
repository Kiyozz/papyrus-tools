//! `PAP002 flags/placement` : un flag sur un objet que le `.flg` refuse.
//!
//! Le compilateur le rejette aussi, mais seulement à la compilation ; dans
//! l'éditeur, le signaler à la frappe évite l'aller-retour.
//!
//! Une property `Auto` accepte aussi les flags de variable : le compilateur
//! génère une variable cachée qui les porte. Le vanilla de Fallout 4 écrit
//! ainsi `Auto Conditional` sur des properties, alors que le `.flg` réserve
//! `Conditional` aux scripts et aux variables.

use papyrus_dialect::GameId;
use papyrus_project::flags::{FlagDef, FlagTarget, parse_flags};
use papyrus_syntax::{SyntaxKind, SyntaxNode};

use crate::{Context, FLAGS_PLACEMENT, Finding};

/// `TESV_Papyrus_Flags.flg`, réduit à ce qui compte.
const SKYRIM: &str = "\
Flag Hidden 0 { Script Property }
Flag Conditional 1 { Script Variable }";

/// `Institute_Papyrus_Flags.flg` et `Starfield_Papyrus_Flags.flg`, identiques.
const FALLOUT4: &str = "\
Flag Hidden 0 { Script Property StructVar }
Flag Conditional 1 { Script Variable }
Flag Default 2 { Script }
Flag CollapsedOnRef 3 { Group }
Flag CollapsedOnBase 4 { Group }
Flag Collapsed CollapsedOnRef & CollapsedOnBase
Flag Mandatory 5 { Property }";

/// Les flags vanilla de `game`.
///
/// Sans cible, ceux de Fallout 4, qui englobent ceux de Skyrim : un flag
/// refusé là l'est partout.
pub fn vanilla_flags(game: Option<GameId>) -> Vec<FlagDef> {
    let text = match game {
        Some(GameId::Skyrim) => SKYRIM,
        _ => FALLOUT4,
    };
    parse_flags(text).expect("table embarquée valide")
}

/// Les objets que désigne une liste de flags, et comment les nommer.
fn targets(list: &SyntaxNode) -> Option<(&'static [FlagTarget], &'static str)> {
    let owner = list.parent()?;
    Some(match owner.kind() {
        SyntaxKind::SCRIPT_HEADER => (&[FlagTarget::Script], "un script"),
        SyntaxKind::PROPERTY_DECL => {
            let auto = list
                .descendants_with_tokens()
                .any(|e| matches!(e.kind(), SyntaxKind::AUTO_KW | SyntaxKind::AUTOREADONLY_KW));
            if auto {
                (
                    &[FlagTarget::Property, FlagTarget::Variable],
                    "une property",
                )
            } else {
                (&[FlagTarget::Property], "une property")
            }
        }
        SyntaxKind::VARIABLE_DECL => {
            if owner.parent()?.kind() == SyntaxKind::STMT_LIST {
                (&[], "une variable locale")
            } else {
                (&[FlagTarget::Variable], "une variable")
            }
        }
        SyntaxKind::STRUCT_FIELD => (&[FlagTarget::StructVar], "un champ de structure"),
        SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL => {
            (&[FlagTarget::Function], "une fonction")
        }
        SyntaxKind::GROUP_DECL => (&[FlagTarget::Group], "un groupe"),
        _ => return None,
    })
}

pub(super) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    for flag in context
        .root
        .descendants()
        .filter(|n| n.kind() == SyntaxKind::FLAG)
    {
        let Some(token) = flag.first_token() else {
            continue;
        };
        // Les flags du langage (`Native`, `Auto`…) ne sont pas dans le `.flg`.
        let Some(def) = context
            .flags
            .iter()
            .find(|d| d.name.eq_ignore_ascii_case(token.text()))
        else {
            continue;
        };
        let Some((allowed_here, what)) = flag.parent().as_ref().and_then(targets) else {
            continue;
        };
        if allowed_here.iter().any(|t| def.targets.contains(t)) {
            continue;
        }
        let permitted: Vec<&str> = def.targets.iter().map(|t| t.name()).collect();
        findings.push(Finding {
            lint: &FLAGS_PLACEMENT,
            range: token.text_range(),
            message: format!(
                "`{}` n'est pas permis sur {what} — permis sur : {}",
                token.text(),
                permitted.join(", ")
            ),
            fix: None,
        });
    }
}
