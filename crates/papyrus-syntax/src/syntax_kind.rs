//! Le vocabulaire de l'arbre syntaxique.
//!
//! `SyntaxKind` est le contrat entre le lexer, le parser, le formateur et le
//! serveur LSP. Il est `#[repr(u16)]` parce que `rowan` stocke les nœuds sous
//! cette forme.
//!
//! Il couvre pour l'instant les *tokens* seuls ; les nœuds arriveront avec le
//! parser.

use papyrus_dialect::GameId;

/// Un token, ou plus tard un nœud, de l'arbre Papyrus.
///
/// Les noms suivent la convention de `rust-analyzer` : majuscules pour les
/// éléments de l'arbre, suffixe `_KW` pour les mots-clés.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[repr(u16)]
#[allow(non_camel_case_types)]
pub enum SyntaxKind {
    // ── Trivia ────────────────────────────────────────────────────────────
    //
    // Tout ce que le parser ignore mais que l'arbre conserve, sans quoi ni le
    // formateur ni le LSP ne peuvent fonctionner.
    /// Espaces et tabulations. Jamais un retour à la ligne.
    WHITESPACE,
    /// `; commentaire jusqu'à la fin de la ligne`, retour à la ligne **exclu**.
    LINE_COMMENT,
    /// `;/ commentaire bloc /;`, éventuellement sur plusieurs lignes.
    BLOCK_COMMENT,
    /// `{ documentation }` attachée à une déclaration.
    DOC_COMMENT,
    /// `\` suivi du retour à la ligne : la déclaration continue en dessous.
    ///
    /// C'est de la trivia, mais le formateur ne doit jamais la supprimer — elle
    /// porte la structure de la ligne logique.
    LINE_CONTINUATION,

    // ── Fin de ligne ──────────────────────────────────────────────────────
    /// Retour à la ligne.
    ///
    /// **N'est pas de la trivia.** Papyrus est orienté ligne : la fin de ligne
    /// termine les déclarations comme le point-virgule ailleurs. C'est la
    /// contrainte qui structure toute la grammaire.
    NEWLINE,

    // ── Littéraux et identifiants ─────────────────────────────────────────
    /// Entier décimal.
    INT_NUMBER,
    /// Entier hexadécimal, `0xDEADBEEF`.
    HEX_NUMBER,
    /// Flottant. Exige des chiffres des deux côtés du point : ni `.5`, ni `1.`,
    /// ni notation exponentielle.
    FLOAT_NUMBER,
    /// Chaîne entre guillemets.
    STRING,
    /// Identifiant, ou mot-clé d'un jeu plus récent que la cible.
    IDENT,

    // ── Ponctuation ───────────────────────────────────────────────────────
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
    /// `:` — séparateur de namespace, Fallout 4 et au-delà.
    COLON,

    // ── Mots-clés présents dès Skyrim ─────────────────────────────────────
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
    /// `As` — le cast. Présent depuis Skyrim, contrairement à `Is`.
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

    // ── Mots-clés ajoutés par Fallout 4 ───────────────────────────────────
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
    /// `Is` — test de type. Absent de Skyrim.
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

    // ── Mots-clés ajoutés par Starfield ───────────────────────────────────
    /// `Guard` — déclaration de verrou.
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

    // ── Bornes ────────────────────────────────────────────────────────────
    /// Suite d'octets que le lexer n'a pas su reconnaître.
    ///
    /// Toujours produite plutôt qu'ignorée : sans elle, l'arbre ne se
    /// reconstruirait pas à l'identique.
    ERROR,
    /// Fin de fichier. Jamais produite par le lexer, ajoutée par le parser.
    EOF,
}

impl SyntaxKind {
    /// Vrai pour les tokens que le parser saute.
    ///
    /// [`SyntaxKind::NEWLINE`] n'en fait pas partie : la fin de ligne porte du
    /// sens en Papyrus.
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

    /// Vrai pour les commentaires, sous toutes leurs formes.
    pub const fn is_comment(self) -> bool {
        matches!(
            self,
            SyntaxKind::LINE_COMMENT | SyntaxKind::BLOCK_COMMENT | SyntaxKind::DOC_COMMENT
        )
    }

    /// Jeu à partir duquel ce token est un mot-clé, ou `None` s'il n'en est pas un.
    pub fn keyword_since(self) -> Option<GameId> {
        crate::keywords::since(self)
    }

    /// Vrai si ce token est un mot-clé dans au moins un jeu.
    pub fn is_keyword(self) -> bool {
        self.keyword_since().is_some()
    }
}

impl From<SyntaxKind> for u16 {
    fn from(kind: SyntaxKind) -> u16 {
        kind as u16
    }
}
