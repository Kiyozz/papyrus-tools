//! The Papyrus grammar, superset of the three engines.
//!
//! Each function reads one construct and emits its events. The rules that
//! only hold for some games come down to two mechanisms:
//!
//! - recent keywords are only recognized if the target reserves them (see
//!   [`crate::token_source`]) — nothing to do here;
//! - constructs that do not start with a keyword, such as namespaces, are
//!   always read, and reported if the target does not accept them. The tree
//!   stays complete in both cases.
//!
//! # Error recovery
//!
//! Three levels, from finest to widest:
//!
//! 1. [`Parser::expect`] reports a missing token without consuming anything;
//! 2. [`eol`] anchors on the line: whatever lingers before the line end goes
//!    into an [`ERROR_TREE`], and the next line starts afresh;
//! 3. [`STMT_RECOVERY`] anchors on blocks: a statement list stops on any
//!    word that closes a block or opens a declaration, at any depth, so a
//!    forgotten `EndIf` does not carry away the file.

mod decls;
mod exprs;
mod stmts;

use papyrus_dialect::Feature;

use super::{Parser, TokenSet};
use crate::SyntaxKind::{self, *};

pub(super) use decls::source_file;

/// Tokens that can start a type.
///
/// `CustomEventName`, `ScriptEventName` and `StructVarName` are keywords
/// *and* types: `ScriptObject.psc` declares
/// `RegisterForCustomEvent(ScriptObject akSender, CustomEventName asEventName)`.
const TYPE_START: TokenSet = TokenSet::new(&[
    INT_KW,
    FLOAT_KW,
    BOOL_KW,
    STRING_KW,
    VAR_KW,
    CUSTOMEVENTNAME_KW,
    SCRIPTEVENTNAME_KW,
    STRUCTVARNAME_KW,
    IDENT,
]);

/// Script, declaration and function flags, all games combined.
///
/// The grammar does not check that a flag is allowed where it appears: that
/// is a lint's job, which can say why.
const FLAGS: TokenSet = TokenSet::new(&[
    AUTO_KW,
    AUTOREADONLY_KW,
    HIDDEN_KW,
    CONDITIONAL_KW,
    NATIVE_KW,
    GLOBAL_KW,
    CONST_KW,
    MANDATORY_KW,
    DEBUGONLY_KW,
    BETAONLY_KW,
    COLLAPSED_KW,
    COLLAPSEDONREF_KW,
    COLLAPSEDONBASE_KW,
    DEFAULT_KW,
    PRIVATE_KW,
    PROTECTED_KW,
    SELFONLY_KW,
    REQUIRESGUARD_KW,
    PROTECTSFUNCTIONLOGIC_KW,
]);

/// The words on which every statement list stops.
///
/// None can start a statement: stopping on them never cuts valid code, and
/// each construct then claims its own terminator.
const STMT_RECOVERY: TokenSet = TokenSet::new(&[
    EOF,
    ENDFUNCTION_KW,
    ENDEVENT_KW,
    ENDIF_KW,
    ELSEIF_KW,
    ELSE_KW,
    ENDWHILE_KW,
    ENDPROPERTY_KW,
    ENDSTATE_KW,
    ENDSTRUCT_KW,
    ENDGROUP_KW,
    ENDLOCKGUARD_KW,
    ELSETRYLOCKGUARD_KW,
    ENDTRYLOCKGUARD_KW,
    FUNCTION_KW,
    EVENT_KW,
    PROPERTY_KW,
    STATE_KW,
    STRUCT_KW,
    GROUP_KW,
    CUSTOMEVENT_KW,
    SCRIPTNAME_KW,
    IMPORT_KW,
]);

/// What [`flag_list`] saw that changes the shape of the declaration.
#[derive(Debug, Clone, Copy, Default)]
struct Flags {
    /// `Auto` or `AutoReadOnly`: the property has no body.
    auto: bool,
    /// `Native`: the function has no body.
    native: bool,
}

/// Consumes the line end, or puts whatever lingers before it into an error.
fn eol(p: &mut Parser) {
    if p.eat(NEWLINE) || p.at(EOF) {
        return;
    }
    p.error("expected end of line");
    skip_line(p);
    p.eat(NEWLINE);
}

/// Puts the rest of the line into an [`ERROR_TREE`].
///
/// Also stops on [`STMT_RECOVERY`]: an `EndFunction` stuck to invalid text
/// must still close its function.
fn skip_line(p: &mut Parser) {
    if p.at(NEWLINE) || p.at_ts(STMT_RECOVERY) {
        return;
    }
    let m = p.start();
    while !p.at(NEWLINE) && !p.at_ts(STMT_RECOVERY) {
        p.bump_any();
    }
    m.complete(p, ERROR_TREE);
}

/// Reports an unintelligible line and skips it entirely.
///
/// Always consumes at least one token, which guarantees the calling loop
/// makes progress.
fn error_line(p: &mut Parser, message: &str) {
    p.error(message);
    let m = p.start();
    p.bump_any();
    while !p.at(NEWLINE) && !p.at_ts(STMT_RECOVERY) {
        p.bump_any();
    }
    m.complete(p, ERROR_TREE);
    p.eat(NEWLINE);
}

/// A declaration name: a simple identifier.
///
/// Flags being reserved only in flag position, they are accepted as names —
/// vanilla writes `Auto State Default` and `Group Mandatory`.
fn name(p: &mut Parser) {
    if p.at(IDENT) || p.at_ts(FLAGS) {
        let m = p.start();
        p.bump_any();
        m.complete(p, NAME);
    } else {
        p.error("expected a name");
    }
}

/// A script name, possibly qualified: `DLC01:MyScript`.
fn script_name(p: &mut Parser) {
    if !p.at(IDENT) {
        p.error("expected a script name");
        return;
    }
    let m = p.start();
    p.bump(IDENT);
    namespace_tail(p);
    m.complete(p, NAME);
}

/// Repeated `:Segment`, with the compatibility diagnostic.
fn namespace_tail(p: &mut Parser) {
    while p.at(COLON) && p.nth(1) == IDENT {
        if !p.dialect().supports(Feature::Namespace) {
            let range = p.current_range();
            p.error_at(range, incompatible(p, Feature::Namespace));
        }
        p.bump(COLON);
        p.bump(IDENT);
    }
}

/// The message for a construct the target does not know.
fn incompatible(p: &Parser, feature: Feature) -> String {
    format!(
        "{}: not available in {} (since {})",
        feature.describe(),
        p.dialect().game.display_name(),
        feature.since().display_name()
    )
}

/// True if a type starts here.
fn at_type(p: &Parser) -> bool {
    p.at_ts(TYPE_START)
}

/// `int`, `Foo:Bar`, `Actor[]`.
fn type_ref(p: &mut Parser) {
    let m = p.start();
    type_base(p);
    if p.at(L_BRACK) && p.nth(1) == R_BRACK {
        p.bump(L_BRACK);
        p.bump(R_BRACK);
    }
    m.complete(p, TYPE_REF);
}

/// A type without array suffix, for `new float[4]` where the brackets carry
/// the size.
fn type_ref_without_array(p: &mut Parser) {
    let m = p.start();
    type_base(p);
    m.complete(p, TYPE_REF);
}

fn type_base(p: &mut Parser) {
    if at_type(p) {
        let simple = !p.at(IDENT);
        p.bump_any();
        if !simple {
            namespace_tail(p);
        }
    } else {
        p.error("expected a type");
    }
}

/// Length, in tokens, of the type starting at the `n`-th token — or `None`.
///
/// Tells `Actor a` (declaration) from `a.b()` or `a = 1` (statements)
/// without consuming anything.
fn type_len(p: &Parser, n: usize) -> Option<usize> {
    let mut len = match p.nth(n) {
        INT_KW | FLOAT_KW | BOOL_KW | STRING_KW | VAR_KW | CUSTOMEVENTNAME_KW
        | SCRIPTEVENTNAME_KW | STRUCTVARNAME_KW => 1,
        IDENT => {
            let mut len = 1;
            while p.nth(n + len) == COLON && p.nth(n + len + 1) == IDENT {
                len += 2;
            }
            len
        }
        _ => return None,
    };
    if p.nth(n + len) == L_BRACK && p.nth(n + len + 1) == R_BRACK {
        len += 2;
    }
    Some(len)
}

/// Sequence of flags, possibly empty. `RequiresGuard(g)` carries arguments.
fn flag_list(p: &mut Parser) -> Flags {
    let mut flags = Flags::default();
    let list = p.start();
    while p.at_ts(FLAGS) {
        match p.current() {
            AUTO_KW | AUTOREADONLY_KW => flags.auto = true,
            NATIVE_KW => flags.native = true,
            _ => {}
        }
        let m = p.start();
        p.bump_any();
        if p.at(L_PAREN) {
            exprs::arg_list(p);
        }
        m.complete(p, FLAG);
    }
    list.complete(p, FLAG_LIST);
    flags
}

/// Ends a block: consumes `end` and its line end, or reports it missing.
fn block_end(p: &mut Parser, end: SyntaxKind, what: &str) {
    if p.expect(end, what) {
        eol(p);
    }
}
