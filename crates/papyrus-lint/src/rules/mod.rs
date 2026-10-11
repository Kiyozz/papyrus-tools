//! Les règles.

mod encoding;
mod flags;
mod naming;

pub use flags::vanilla_flags;

use crate::{Context, Finding};

pub(crate) fn run(context: &Context<'_>, findings: &mut Vec<Finding>) {
    encoding::run(context, findings);
    flags::run(context, findings);
    naming::run(context, findings);
}
