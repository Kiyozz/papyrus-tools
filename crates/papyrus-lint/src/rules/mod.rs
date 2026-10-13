//! The rules.

mod encoding;
mod events;
mod flags;
mod naming;
mod semantic;

pub use flags::vanilla_flags;

use crate::{Context, Finding};

pub(crate) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    encoding::run(context, findings);
    flags::run(context, findings);
    events::run(context, findings);
    semantic::run(context, findings);
    naming::run(context, findings);
}
