//! `PAP001 encoding/lossy` — voir [`papyrus_project::encoding`].

use papyrus_project::encoding;

use crate::{Context, ENCODING_LOSSY, Finding};

pub(super) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    // Le remède dépend du jeu : sans cible, mieux vaut se taire que mal
    // conseiller.
    let Some(game) = context.game else {
        return;
    };
    if let Some(issue) = encoding::check(context.decoded, game) {
        findings.push(Finding {
            lint: &ENCODING_LOSSY,
            range: issue.range,
            message: issue.message,
            fix: None,
        });
    }
}
