//! `PAP001 encoding/lossy` — see [`papyrus_project::encoding`].

use papyrus_project::encoding;

use crate::{Context, ENCODING_LOSSY, Finding};

pub(super) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    // The remedy depends on the game: without a target, better stay quiet
    // than give bad advice.
    let Some(game) = context.game else {
        return;
    };
    if let Some(issue) = encoding::check(context.decoded, game) {
        findings.push(Finding {
            lint: &ENCODING_LOSSY,
            range: issue.range,
            message: issue.message,
            fix: None,
            unsafe_fix: None,
            suggestions: Vec::new(),
        });
    }
}
