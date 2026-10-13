//! `PAP002 flags/placement`: a flag on an object the `.flg` refuses.
//!
//! The compiler rejects it too, but only when compiling; in the editor,
//! reporting it as you type saves the round trip.
//!
//! An `Auto` property also accepts variable flags: the compiler generates a
//! hidden variable that carries them. Fallout 4's vanilla thus writes
//! `Auto Conditional` on properties, although the `.flg` reserves
//! `Conditional` for scripts and variables.

use papyrus_dialect::GameId;
use papyrus_project::flags::{FlagDef, FlagTarget, parse_flags};
use papyrus_syntax::{SyntaxKind, SyntaxNode};

use crate::{Context, FLAGS_PLACEMENT, Finding};

/// `TESV_Papyrus_Flags.flg`, reduced to what matters.
const SKYRIM: &str = "\
Flag Hidden 0 { Script Property }
Flag Conditional 1 { Script Variable }";

/// `Institute_Papyrus_Flags.flg` and `Starfield_Papyrus_Flags.flg`, identical.
const FALLOUT4: &str = "\
Flag Hidden 0 { Script Property StructVar }
Flag Conditional 1 { Script Variable }
Flag Default 2 { Script }
Flag CollapsedOnRef 3 { Group }
Flag CollapsedOnBase 4 { Group }
Flag Collapsed CollapsedOnRef & CollapsedOnBase
Flag Mandatory 5 { Property }";

/// The vanilla flags of `game`.
///
/// Without a target, Fallout 4's, which include Skyrim's: a flag refused
/// there is refused everywhere.
pub fn vanilla_flags(game: Option<GameId>) -> Vec<FlagDef> {
    let text = match game {
        Some(GameId::Skyrim) => SKYRIM,
        _ => FALLOUT4,
    };
    parse_flags(text).expect("valid embedded table")
}

/// The objects a flag list applies to, and how to name them.
fn targets(list: &SyntaxNode) -> Option<(&'static [FlagTarget], &'static str)> {
    let owner = list.parent()?;
    Some(match owner.kind() {
        SyntaxKind::SCRIPT_HEADER => (&[FlagTarget::Script], "a script"),
        SyntaxKind::PROPERTY_DECL => {
            let auto = list
                .descendants_with_tokens()
                .any(|e| matches!(e.kind(), SyntaxKind::AUTO_KW | SyntaxKind::AUTOREADONLY_KW));
            if auto {
                (&[FlagTarget::Property, FlagTarget::Variable], "a property")
            } else {
                (&[FlagTarget::Property], "a property")
            }
        }
        SyntaxKind::VARIABLE_DECL => {
            if owner.parent()?.kind() == SyntaxKind::STMT_LIST {
                (&[], "a local variable")
            } else {
                (&[FlagTarget::Variable], "a variable")
            }
        }
        SyntaxKind::STRUCT_FIELD => (&[FlagTarget::StructVar], "a struct field"),
        SyntaxKind::FUNCTION_DECL | SyntaxKind::EVENT_DECL => {
            (&[FlagTarget::Function], "a function")
        }
        SyntaxKind::GROUP_DECL => (&[FlagTarget::Group], "a group"),
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
        // Language flags (`Native`, `Auto`…) are not in the `.flg`.
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
                "`{}` is not allowed on {what} — allowed on: {}",
                token.text(),
                permitted.join(", ")
            ),
            fix: None,
            unsafe_fix: None,
            suggestion: None,
        });
    }
}
