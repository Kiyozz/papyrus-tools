//! The opcodes, in the order of their codes, named as `PapyrusAssembler` names
//! them.

/// An opcode.
#[derive(Debug, PartialEq, Eq)]
pub struct Op {
    /// As an assembly file writes it.
    pub name: &'static str,
    /// How many arguments always follow it.
    pub args: u8,
    /// A count and that many arguments follow them.
    pub variadic: bool,
}

const fn fixed(name: &'static str, args: u8) -> Op {
    Op {
        name,
        args,
        variadic: false,
    }
}

const fn variadic(name: &'static str, args: u8) -> Op {
    Op {
        name,
        args,
        variadic: true,
    }
}

static OPS: [Op; 51] = [
    fixed("Nop", 0),
    fixed("IAdd", 3),
    fixed("FAdd", 3),
    fixed("ISubtract", 3),
    fixed("FSubtract", 3),
    fixed("IMultiply", 3),
    fixed("FMultiply", 3),
    fixed("IDivide", 3),
    fixed("FDivide", 3),
    fixed("IMod", 3),
    fixed("Not", 2),
    fixed("INegate", 2),
    fixed("FNegate", 2),
    fixed("Assign", 2),
    fixed("Cast", 2),
    fixed("CompareEQ", 3),
    fixed("CompareLT", 3),
    fixed("CompareLTE", 3),
    fixed("CompareGT", 3),
    fixed("CompareGTE", 3),
    fixed("Jump", 1),
    fixed("JumpT", 2),
    fixed("JumpF", 2),
    variadic("CallMethod", 3),
    variadic("CallParent", 2),
    variadic("CallStatic", 3),
    fixed("Return", 1),
    fixed("StrCat", 3),
    fixed("PropGet", 3),
    fixed("PropSet", 3),
    fixed("ArrayCreate", 2),
    fixed("ArrayLength", 2),
    fixed("ArrayGetElement", 3),
    fixed("ArraySetElement", 3),
    fixed("ArrayFindElement", 4),
    fixed("ArrayRFindElement", 4),
    // Fallout 4.
    fixed("Is", 3),
    fixed("StructCreate", 1),
    fixed("StructGet", 3),
    fixed("StructSet", 3),
    fixed("ArrayFindStruct", 5),
    fixed("ArrayRFindStruct", 5),
    fixed("ArrayAddElements", 3),
    fixed("ArrayInsertElement", 3),
    fixed("ArrayRemoveLastElement", 1),
    fixed("ArrayRemoveElements", 3),
    fixed("ArrayClearElements", 1),
    // Fallout 76.
    fixed("ArrayGetAllMatchingStructs", 6),
    // Starfield.
    variadic("LockGuards", 0),
    variadic("UnlockGuards", 0),
    variadic("TryLockGuards", 1),
];

impl Op {
    /// The opcode `code`, if `game` has it.
    pub(crate) fn of(code: u8, game: u16) -> Option<&'static Self> {
        let count = match game {
            1 => 36,
            2 => 47,
            3 => 48,
            _ => OPS.len(),
        };
        OPS[..count].get(usize::from(code))
    }

    /// Whether its last argument is a jump, relative to its own position.
    pub(crate) fn jumps(&self) -> bool {
        matches!(self.name, "Jump" | "JumpT" | "JumpF")
    }
}
