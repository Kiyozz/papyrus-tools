//! What the compiler provides without a script declaring it: the functions
//! of arrays, and Skyrim's state functions, which every script has but no
//! native script declares.
//!
//! They take the form of native scripts, so that hovers and completions
//! treat them like any other.

use papyrus_stdlib::{Function, Param, Script};

const fn param(name: &'static str, ty: &'static str, default: Option<&'static str>) -> Param {
    Param { name, ty, default }
}

const fn function(
    name: &'static str,
    returns: Option<&'static str>,
    params: &'static [Param],
) -> Function {
    Function {
        name,
        returns,
        params,
        global: false,
        native: true,
    }
}

/// The functions of an array. Skyrim only knows the first two.
pub(crate) static ARRAY: Script = Script {
    name: "array",
    parent: None,
    functions: &[
        function(
            "Find",
            Some("int"),
            &[
                param("akElement", "var", None),
                param("aiStartIndex", "int", Some("0")),
            ],
        ),
        function(
            "RFind",
            Some("int"),
            &[
                param("akElement", "var", None),
                param("aiStartIndex", "int", Some("-1")),
            ],
        ),
        function(
            "Add",
            None,
            &[
                param("akElement", "var", None),
                param("aiCount", "int", Some("1")),
            ],
        ),
        function(
            "Insert",
            None,
            &[
                param("akElement", "var", None),
                param("aiLocation", "int", None),
            ],
        ),
        function(
            "Remove",
            None,
            &[
                param("aiLocation", "int", None),
                param("aiCount", "int", Some("1")),
            ],
        ),
        function("RemoveLast", None, &[]),
        function("Clear", None, &[]),
        function(
            "FindStruct",
            Some("int"),
            &[
                param("asVarName", "string", None),
                param("akElement", "var", None),
                param("aiStartIndex", "int", Some("0")),
            ],
        ),
        function(
            "RFindStruct",
            Some("int"),
            &[
                param("asVarName", "string", None),
                param("akElement", "var", None),
                param("aiStartIndex", "int", Some("-1")),
            ],
        ),
        function(
            "GetAllMatchingStructs",
            Some("var[]"),
            &[
                param("asVarName", "string", None),
                param("akElement", "var", None),
                param("aiStartIndex", "int", Some("0")),
                param("aiEndIndex", "int", Some("-1")),
            ],
        ),
    ],
    events: &[],
    properties: &[],
};

/// The number of array functions Skyrim knows.
pub(crate) const SKYRIM_ARRAY_FUNCTIONS: usize = 2;

/// Skyrim's functions and events common to every script. From Fallout 4
/// on, `ScriptObject` declares them.
pub(crate) static SKYRIM_SCRIPT: Script = Script {
    name: "built-in",
    parent: None,
    functions: &[
        function("GetState", Some("string"), &[]),
        function("GotoState", None, &[param("asNewState", "string", None)]),
    ],
    events: &[
        function("OnBeginState", None, &[]),
        function("OnEndState", None, &[]),
        function("OnInit", None, &[]),
    ],
    properties: &[],
};
