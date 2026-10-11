//! The vocabulary of the syntax tree.
//!
//! `SyntaxKind` is the contract between the lexer, the parser, the formatter
//! and the language server. It is `#[repr(u16)]` because `rowan` stores nodes
//! in that form.
//!
//! *Tokens* come first, *nodes* after: the border between the two is the
//! discriminant of [`SyntaxKind::SOURCE_FILE`], which makes
//! [`SyntaxKind::is_node`] free.

use papyrus_dialect::GameId;

/// Declares the enumeration and the table that rebuilds it.
///
/// `rowan` stores kinds as `u16`; we therefore need to go back from `u16` to
/// `SyntaxKind`. `rust-analyzer` does it with `transmute`; this crate forbids
/// `unsafe` code, hence this generated table. Writing the list twice by hand
/// would guarantee they eventually diverge.
macro_rules! syntax_kinds {
    ($( $(#[$attr:meta])* $name:ident ),* $(,)?) => {
        /// A token or a node of the Papyrus tree.
        ///
        /// Names follow the `rust-analyzer` convention: uppercase for tree
        /// elements, `_KW` suffix for keywords.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
        #[repr(u16)]
        #[allow(non_camel_case_types)]
        pub enum SyntaxKind {
            $( $(#[$attr])* $name, )*
        }

        impl SyntaxKind {
            /// All kinds, in discriminant order.
            pub(crate) const ALL: &'static [SyntaxKind] = &[ $( SyntaxKind::$name, )* ];
        }
    };
}

syntax_kinds! {
    // ══ Tokens ════════════════════════════════════════════════════════════

    // ── Trivia ────────────────────────────────────────────────────────────
    //
    // Everything the parser ignores but the tree keeps, without which neither
    // the formatter nor the language server can work.
    /// Spaces and tabs. Never a line break.
    WHITESPACE,
    /// `; comment up to the end of the line`, line break **excluded**.
    LINE_COMMENT,
    /// `;/ block comment /;`, possibly spanning several lines.
    BLOCK_COMMENT,
    /// `{ documentation }` attached to a declaration.
    DOC_COMMENT,
    /// `\` followed by the line break: the declaration continues below.
    ///
    /// It is trivia, but the formatter must never remove it — it carries the
    /// structure of the logical line.
    LINE_CONTINUATION,

    // ── Line end ──────────────────────────────────────────────────────────
    /// Line break.
    ///
    /// **Not trivia.** Papyrus is line-oriented: the line end terminates
    /// declarations like the semicolon elsewhere. That constraint shapes the
    /// whole grammar.
    NEWLINE,

    // ── Literals and identifiers ──────────────────────────────────────────
    /// Decimal integer.
    INT_NUMBER,
    /// Hexadecimal integer, `0xDEADBEEF`.
    HEX_NUMBER,
    /// Float. Requires digits on both sides of the dot: no `.5`, no `1.`, no
    /// exponent notation.
    FLOAT_NUMBER,
    /// Double-quoted string.
    STRING,
    /// Identifier, or keyword of a game newer than the target.
    IDENT,

    // ── Punctuation ───────────────────────────────────────────────────────
    /// `+`
    PLUS,
    /// `-`
    MINUS,
    /// `*`
    STAR,
    /// `/`
    SLASH,
    /// `%`
    PERCENT,
    /// `+=`
    PLUS_EQ,
    /// `-=`
    MINUS_EQ,
    /// `*=`
    STAR_EQ,
    /// `/=`
    SLASH_EQ,
    /// `%=`
    PERCENT_EQ,
    /// `=`
    EQ,
    /// `==`
    EQ2,
    /// `!=`
    NEQ,
    /// `<`
    LT,
    /// `>`
    GT,
    /// `<=`
    LTEQ,
    /// `>=`
    GTEQ,
    /// `&&`
    AMP2,
    /// `||`
    PIPE2,
    /// `!`
    BANG,
    /// `.`
    DOT,
    /// `,`
    COMMA,
    /// `(`
    L_PAREN,
    /// `)`
    R_PAREN,
    /// `[`
    L_BRACK,
    /// `]`
    R_BRACK,
    /// `:` — namespace separator, Fallout 4 and later.
    COLON,

    // ── Keywords present since Skyrim ─────────────────────────────────────
    /// `ScriptName`
    SCRIPTNAME_KW,
    /// `Extends`
    EXTENDS_KW,
    /// `Import`
    IMPORT_KW,
    /// `Function`
    FUNCTION_KW,
    /// `EndFunction`
    ENDFUNCTION_KW,
    /// `Event`
    EVENT_KW,
    /// `EndEvent`
    ENDEVENT_KW,
    /// `Property`
    PROPERTY_KW,
    /// `EndProperty`
    ENDPROPERTY_KW,
    /// `State`
    STATE_KW,
    /// `EndState`
    ENDSTATE_KW,
    /// `If`
    IF_KW,
    /// `ElseIf`
    ELSEIF_KW,
    /// `Else`
    ELSE_KW,
    /// `EndIf`
    ENDIF_KW,
    /// `While`
    WHILE_KW,
    /// `EndWhile`
    ENDWHILE_KW,
    /// `Return`
    RETURN_KW,
    /// `Auto`
    AUTO_KW,
    /// `AutoReadOnly`
    AUTOREADONLY_KW,
    /// `Hidden`
    HIDDEN_KW,
    /// `Conditional`
    CONDITIONAL_KW,
    /// `Native`
    NATIVE_KW,
    /// `Global`
    GLOBAL_KW,
    /// `New`
    NEW_KW,
    /// `As` — the cast. Present since Skyrim, unlike `Is`.
    AS_KW,
    /// `Self`
    SELF_KW,
    /// `Parent`
    PARENT_KW,
    /// `None`
    NONE_KW,
    /// `Length`
    LENGTH_KW,
    /// `True`
    TRUE_KW,
    /// `False`
    FALSE_KW,
    /// `Int`
    INT_KW,
    /// `Float`
    FLOAT_KW,
    /// `Bool`
    BOOL_KW,
    /// `String`
    STRING_KW,

    // ── Keywords added by Fallout 4 ───────────────────────────────────────
    /// `Struct`
    STRUCT_KW,
    /// `EndStruct`
    ENDSTRUCT_KW,
    /// `Group`
    GROUP_KW,
    /// `EndGroup`
    ENDGROUP_KW,
    /// `Var`
    VAR_KW,
    /// `Is` — type test. Absent from Skyrim.
    IS_KW,
    /// `Const`
    CONST_KW,
    /// `DebugOnly`
    DEBUGONLY_KW,
    /// `BetaOnly`
    BETAONLY_KW,
    /// `Mandatory`
    MANDATORY_KW,
    /// `Collapsed`
    COLLAPSED_KW,
    /// `CollapsedOnRef`
    COLLAPSEDONREF_KW,
    /// `CollapsedOnBase`
    COLLAPSEDONBASE_KW,
    /// `Default`
    DEFAULT_KW,
    /// `CustomEvent`
    CUSTOMEVENT_KW,
    /// `ScriptEventName`
    SCRIPTEVENTNAME_KW,
    /// `CustomEventName`
    CUSTOMEVENTNAME_KW,
    /// `StructVarName`
    STRUCTVARNAME_KW,

    // ── Keywords added by Starfield ───────────────────────────────────────
    /// `Guard` — lock declaration.
    GUARD_KW,
    /// `LockGuard`
    LOCKGUARD_KW,
    /// `EndLockGuard`
    ENDLOCKGUARD_KW,
    /// `TryLockGuard`
    TRYLOCKGUARD_KW,
    /// `ElseTryLockGuard`
    ELSETRYLOCKGUARD_KW,
    /// `EndTryLockGuard`
    ENDTRYLOCKGUARD_KW,
    /// `RequiresGuard`
    REQUIRESGUARD_KW,
    /// `ProtectsFunctionLogic`
    PROTECTSFUNCTIONLOGIC_KW,
    /// `Private`
    PRIVATE_KW,
    /// `Protected`
    PROTECTED_KW,
    /// `SelfOnly`
    SELFONLY_KW,

    // ── Bounds ────────────────────────────────────────────────────────────
    /// A run of bytes the lexer could not recognize.
    ///
    /// Always produced rather than dropped: without it, the tree would not
    /// rebuild identically.
    ERROR,
    /// End of file. Never produced by the lexer, added by the parser.
    EOF,

    // ══ Nodes ═════════════════════════════════════════════════════════════
    //
    // `SOURCE_FILE` opens the section: every kind from here on is a node.

    /// The root: a whole `.psc` file.
    SOURCE_FILE,
    /// `ScriptName Foo extends Bar Native Hidden`
    SCRIPT_HEADER,
    /// `extends Foo:Bar`
    EXTENDS_CLAUSE,
    /// `import Foo`
    IMPORT,

    /// Sequence of flags. Always present, possibly empty, so that the
    /// formatter has somewhere to insert.
    FLAG_LIST,
    /// A flag, with its arguments if it takes any: `Const`, `RequiresGuard(g)`.
    FLAG,

    /// Name in declaration position.
    NAME,
    /// Name in use position, `self` and `parent` included.
    NAME_REF,
    /// `Foo:Bar[]`, `int`, `var`
    TYPE_REF,

    /// `(int aiCount = 3, Actor akRef)`
    PARAM_LIST,
    /// `int aiCount = 3`
    PARAM,

    /// Script or local variable: `int x = 1`.
    VARIABLE_DECL,
    /// `int Property Foo = 1 Auto`, short or long form.
    PROPERTY_DECL,
    /// `Function Foo() … EndFunction`
    FUNCTION_DECL,
    /// `Event OnInit() … EndEvent`, including Fallout 4's remote form
    /// `Event ObjectReference.OnActivate(…)`.
    EVENT_DECL,
    /// `Auto State Foo … EndState`
    STATE_DECL,
    /// `Struct Foo … EndStruct`
    STRUCT_DECL,
    /// A struct field: `int Threshold`.
    STRUCT_FIELD,
    /// `Group Foo … EndGroup`
    GROUP_DECL,
    /// `CustomEvent Foo`
    CUSTOM_EVENT_DECL,
    /// `Guard foo ProtectsFunctionLogic`
    GUARD_DECL,

    /// Body of a function, an event or a branch.
    STMT_LIST,
    /// An expression used as a statement — almost always a call.
    EXPR_STMT,
    /// `x = 1`, `x += 1`
    ASSIGN_STMT,
    /// `Return` or `Return expr`
    RETURN_STMT,
    /// `If … ElseIf … Else … EndIf`
    IF_STMT,
    /// `ElseIf cond`
    ELSEIF_CLAUSE,
    /// `Else`
    ELSE_CLAUSE,
    /// `While … EndWhile`
    WHILE_STMT,
    /// `LockGuard g … EndLockGuard`
    LOCK_GUARD_STMT,
    /// `TryLockGuard g … EndTryLockGuard`
    TRY_LOCK_GUARD_STMT,
    /// `ElseTryLockGuard`
    ELSE_TRY_LOCK_GUARD_CLAUSE,
    /// The locks taken by a `LockGuard`.
    GUARD_LIST,

    /// Number, string, `true`, `false`, `none`.
    LITERAL,
    /// `( expr )`
    PAREN_EXPR,
    /// `-x`, `!x`
    UNARY_EXPR,
    /// `a + b`, `a && b`, `a == b`
    BIN_EXPR,
    /// `x as Actor`, `x is Actor`
    CAST_EXPR,
    /// `arr[i]`
    INDEX_EXPR,
    /// `Foo(a, b)`
    CALL_EXPR,
    /// `a.b`
    FIELD_EXPR,
    /// `new float[4]`, `new Foo:Bar`
    NEW_EXPR,
    /// `(a, abForce = true)`
    ARG_LIST,
    /// An argument, named or positional.
    ARG,

    /// A portion the parser could not interpret.
    ///
    /// Skipped tokens are stored there rather than lost: the tree stays
    /// rebuildable, and the formatter knows which area not to touch.
    ERROR_TREE,

    /// Node opened then abandoned. Never appears in the final tree.
    TOMBSTONE,
}

impl SyntaxKind {
    /// Rebuilds a kind from the `u16` stored by `rowan`.
    pub fn from_raw(raw: u16) -> Option<SyntaxKind> {
        Self::ALL.get(raw as usize).copied()
    }

    /// True for an inner node, false for a token.
    pub const fn is_node(self) -> bool {
        (self as u16) >= (SyntaxKind::SOURCE_FILE as u16)
    }

    /// True for the tokens the parser skips.
    ///
    /// [`SyntaxKind::NEWLINE`] is not one of them: the line end carries
    /// meaning in Papyrus.
    pub const fn is_trivia(self) -> bool {
        matches!(
            self,
            SyntaxKind::WHITESPACE
                | SyntaxKind::LINE_COMMENT
                | SyntaxKind::BLOCK_COMMENT
                | SyntaxKind::DOC_COMMENT
                | SyntaxKind::LINE_CONTINUATION
        )
    }

    /// True for comments, in all their forms.
    pub const fn is_comment(self) -> bool {
        matches!(
            self,
            SyntaxKind::LINE_COMMENT | SyntaxKind::BLOCK_COMMENT | SyntaxKind::DOC_COMMENT
        )
    }

    /// The game from which this token is a keyword, or `None` if it is not one.
    pub fn keyword_since(self) -> Option<GameId> {
        crate::keywords::since(self)
    }

    /// True if this token is a keyword in at least one game.
    pub fn is_keyword(self) -> bool {
        self.keyword_since().is_some()
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> u16 {
        kind as u16
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_raw_est_l_inverse_du_discriminant() {
        for (index, kind) in SyntaxKind::ALL.iter().enumerate() {
            assert_eq!(u16::from(*kind) as usize, index, "{kind:?} mal placé");
            assert_eq!(SyntaxKind::from_raw(index as u16), Some(*kind));
        }
        assert_eq!(SyntaxKind::from_raw(SyntaxKind::ALL.len() as u16), None);
    }

    #[test]
    fn tokens_et_noeuds_ne_se_melangent_pas() {
        // The border is a plain `>=` on the discriminant; provided no token
        // slipped in after it.
        assert!(!SyntaxKind::EOF.is_node());
        assert!(!SyntaxKind::IDENT.is_node());
        assert!(SyntaxKind::SOURCE_FILE.is_node());
        assert!(SyntaxKind::ERROR_TREE.is_node());

        for kind in SyntaxKind::ALL {
            if kind.is_node() {
                assert!(!kind.is_trivia(), "{kind:?} est un nœud et de la trivia");
                assert!(!kind.is_keyword(), "{kind:?} est un nœud et un mot-clé");
            }
        }
    }
}
