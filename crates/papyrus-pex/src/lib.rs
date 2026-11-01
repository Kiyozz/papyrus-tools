//! `.pex` files, compiled Papyrus scripts.
//!
//! Skyrim writes them big-endian, Fallout 4, Fallout 76 and Starfield
//! little-endian; the magic number tells which. Later games add a few sections
//! and opcodes, on top of an otherwise identical layout. [`Pex::read`] takes
//! the bytes of a file that may be damaged or from a mod: it fails with the
//! offset of what it could not read, and does not panic.

mod disassemble;
mod op;

use std::fmt;

pub use disassemble::disassemble;
pub use op::Op;

/// An index in [`Pex::strings`], checked when read.
pub type Name = u16;

/// The `game` of a Skyrim script, the first to have `.pex` files.
const SKYRIM: u16 = 1;
const FALLOUT4: u16 = 2;
const FALLOUT76: u16 = 3;
const STARFIELD: u16 = 4;

/// What could not be read, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// In bytes, from the start of the file.
    pub offset: usize,
    pub message: String,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} at offset {:#x}", self.message, self.offset)
    }
}

impl std::error::Error for Error {}

/// A compiled script.
#[derive(Debug, Clone, PartialEq)]
pub struct Pex {
    pub major: u8,
    pub minor: u8,
    /// 1 for Skyrim, 2 for Fallout 4, 3 for Fallout 76, 4 for Starfield.
    pub game: u16,
    /// In seconds since 1970.
    pub compile_time: u64,
    pub source: String,
    pub user: String,
    pub computer: String,
    pub strings: Vec<String>,
    /// Absent from a script stripped of it.
    pub debug: Option<Debug>,
    pub user_flags: Vec<UserFlag>,
    pub objects: Vec<Object>,
}

impl Pex {
    /// The string `name` stands for.
    pub fn text(&self, name: Name) -> &str {
        &self.strings[usize::from(name)]
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Debug {
    /// In seconds since 1970.
    pub modify_time: u64,
    pub functions: Vec<DebugFunction>,
    pub property_groups: Vec<PropertyGroup>,
    pub struct_orders: Vec<StructOrder>,
}

/// The source lines of a function's instructions.
#[derive(Debug, Clone, PartialEq)]
pub struct DebugFunction {
    pub object: Name,
    pub state: Name,
    pub function: Name,
    /// 0 for a function, 1 for a getter, 2 for a setter.
    pub kind: u8,
    pub lines: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PropertyGroup {
    pub object: Name,
    pub name: Name,
    pub doc: Name,
    pub user_flags: u32,
    pub properties: Vec<Name>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct StructOrder {
    pub object: Name,
    pub name: Name,
    pub members: Vec<Name>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct UserFlag {
    pub name: Name,
    pub bit: u8,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Object {
    pub name: Name,
    pub parent: Name,
    pub doc: Name,
    pub constant: bool,
    pub user_flags: u32,
    pub auto_state: Name,
    pub structs: Vec<Struct>,
    pub variables: Vec<Variable>,
    pub guards: Vec<Name>,
    pub properties: Vec<Property>,
    pub states: Vec<State>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Struct {
    pub name: Name,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Member {
    pub name: Name,
    pub ty: Name,
    pub user_flags: u32,
    pub value: Value,
    pub constant: bool,
    pub doc: Name,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Variable {
    pub name: Name,
    pub ty: Name,
    pub user_flags: u32,
    pub value: Value,
    pub constant: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Property {
    pub name: Name,
    pub ty: Name,
    pub doc: Name,
    pub user_flags: u32,
    /// The hidden variable of an `Auto` property.
    pub auto_var: Option<Name>,
    pub get: Option<Function>,
    pub set: Option<Function>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct State {
    /// Empty for the default state.
    pub name: Name,
    pub functions: Vec<(Name, Function)>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub returns: Name,
    pub doc: Name,
    pub user_flags: u32,
    pub global: bool,
    pub native: bool,
    pub parameters: Vec<(Name, Name)>,
    pub locals: Vec<(Name, Name)>,
    pub code: Vec<Instruction>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Instruction {
    pub op: &'static Op,
    pub args: Vec<Value>,
    /// The arguments of a call, or the guards of a lock.
    pub rest: Vec<Value>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Value {
    None,
    Identifier(Name),
    String(Name),
    Integer(i32),
    Float(f32),
    Bool(bool),
}

/// Walks the bytes of a file.
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
    big_endian: bool,
    /// How many strings a [`Name`] can point to.
    strings: usize,
}

impl<'a> Reader<'a> {
    fn fail<T>(&self, offset: usize, message: impl Into<String>) -> Result<T, Error> {
        Err(Error {
            offset,
            message: message.into(),
        })
    }

    fn take(&mut self, count: usize) -> Result<&'a [u8], Error> {
        let Some(taken) = self
            .at
            .checked_add(count)
            .and_then(|end| self.bytes.get(self.at..end))
        else {
            return self.fail(self.at, "unexpected end of file");
        };
        self.at += count;
        Ok(taken)
    }

    fn array<const N: usize>(&mut self) -> Result<[u8; N], Error> {
        let mut bytes = [0; N];
        bytes.copy_from_slice(self.take(N)?);
        if self.big_endian {
            bytes.reverse();
        }
        Ok(bytes)
    }

    fn u8(&mut self) -> Result<u8, Error> {
        Ok(self.take(1)?[0])
    }

    fn bool(&mut self) -> Result<bool, Error> {
        Ok(self.u8()? != 0)
    }

    fn u16(&mut self) -> Result<u16, Error> {
        self.array().map(u16::from_le_bytes)
    }

    fn u32(&mut self) -> Result<u32, Error> {
        self.array().map(u32::from_le_bytes)
    }

    fn u64(&mut self) -> Result<u64, Error> {
        self.array().map(u64::from_le_bytes)
    }

    /// A string of the file: its length, then Windows-1252 text.
    fn string(&mut self) -> Result<String, Error> {
        let length = self.u16()?;
        let bytes = self.take(usize::from(length))?;
        let (text, _) = encoding_rs::WINDOWS_1252.decode_without_bom_handling(bytes);
        Ok(text.into_owned())
    }

    fn name(&mut self) -> Result<Name, Error> {
        let at = self.at;
        let name = self.u16()?;
        if usize::from(name) >= self.strings {
            return self.fail(at, format!("no string {name} in the table"));
        }
        Ok(name)
    }

    /// `count` items, stopping at the first that cannot be read.
    fn list<T>(
        &mut self,
        count: usize,
        mut item: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        // The count comes from the file: the items, not it, size the list.
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(item(self)?);
        }
        Ok(items)
    }

    /// A list preceded by its length.
    fn counted<T>(
        &mut self,
        item: impl FnMut(&mut Self) -> Result<T, Error>,
    ) -> Result<Vec<T>, Error> {
        let count = self.u16()?;
        self.list(usize::from(count), item)
    }

    fn value(&mut self) -> Result<Value, Error> {
        let at = self.at;
        Ok(match self.u8()? {
            0 => Value::None,
            // 8 and 9, a type and a guard, come from `PapyrusAssembler -S`.
            1 | 8 | 9 => Value::Identifier(self.name()?),
            2 => Value::String(self.name()?),
            3 => Value::Integer(self.u32()?.cast_signed()),
            4 => Value::Float(f32::from_bits(self.u32()?)),
            5 => Value::Bool(self.bool()?),
            kind => return self.fail(at, format!("unknown value type {kind}")),
        })
    }

    fn debug(&mut self, game: u16) -> Result<Debug, Error> {
        let modify_time = self.u64()?;
        let functions = self.counted(|r| {
            Ok(DebugFunction {
                object: r.name()?,
                state: r.name()?,
                function: r.name()?,
                kind: r.u8()?,
                lines: r.counted(Self::u16)?,
            })
        })?;
        let mut debug = Debug {
            modify_time,
            functions,
            property_groups: Vec::new(),
            struct_orders: Vec::new(),
        };
        if game > SKYRIM {
            debug.property_groups = self.counted(|r| {
                Ok(PropertyGroup {
                    object: r.name()?,
                    name: r.name()?,
                    doc: r.name()?,
                    user_flags: r.u32()?,
                    properties: r.counted(Self::name)?,
                })
            })?;
            debug.struct_orders = self.counted(|r| {
                Ok(StructOrder {
                    object: r.name()?,
                    name: r.name()?,
                    members: r.counted(Self::name)?,
                })
            })?;
        }
        Ok(debug)
    }

    fn object(&mut self, game: u16) -> Result<Object, Error> {
        let name = self.name()?;
        // Its size: the compilers disagree on whether it counts itself, and
        // nothing needs it.
        self.u32()?;
        let parent = self.name()?;
        let doc = self.name()?;
        let constant = game > SKYRIM && self.bool()?;
        let user_flags = self.u32()?;
        let auto_state = self.name()?;
        let structs = if game > SKYRIM {
            self.counted(Self::structure)?
        } else {
            Vec::new()
        };
        let variables = self.counted(|r| r.variable(game))?;
        let guards = if game == STARFIELD {
            self.counted(Self::name)?
        } else {
            Vec::new()
        };
        let properties = self.counted(|r| r.property(game))?;
        let states = self.counted(|r| r.state(game))?;
        Ok(Object {
            name,
            parent,
            doc,
            constant,
            user_flags,
            auto_state,
            structs,
            variables,
            guards,
            properties,
            states,
        })
    }

    fn structure(&mut self) -> Result<Struct, Error> {
        Ok(Struct {
            name: self.name()?,
            members: self.counted(|r| {
                Ok(Member {
                    name: r.name()?,
                    ty: r.name()?,
                    user_flags: r.u32()?,
                    value: r.value()?,
                    constant: r.bool()?,
                    doc: r.name()?,
                })
            })?,
        })
    }

    fn variable(&mut self, game: u16) -> Result<Variable, Error> {
        Ok(Variable {
            name: self.name()?,
            ty: self.name()?,
            user_flags: self.u32()?,
            value: self.value()?,
            constant: game > SKYRIM && self.bool()?,
        })
    }

    fn property(&mut self, game: u16) -> Result<Property, Error> {
        let name = self.name()?;
        let ty = self.name()?;
        let doc = self.name()?;
        let user_flags = self.u32()?;
        let flags = self.u8()?;
        let auto = flags & 4 != 0;
        let auto_var = auto.then(|| self.name()).transpose()?;
        let get = (flags & 1 != 0 && !auto)
            .then(|| self.function(game))
            .transpose()?;
        let set = (flags & 2 != 0 && !auto)
            .then(|| self.function(game))
            .transpose()?;
        Ok(Property {
            name,
            ty,
            doc,
            user_flags,
            auto_var,
            get,
            set,
        })
    }

    fn state(&mut self, game: u16) -> Result<State, Error> {
        Ok(State {
            name: self.name()?,
            functions: self.counted(|r| Ok((r.name()?, r.function(game)?)))?,
        })
    }

    fn pair(&mut self) -> Result<(Name, Name), Error> {
        Ok((self.name()?, self.name()?))
    }

    fn function(&mut self, game: u16) -> Result<Function, Error> {
        let returns = self.name()?;
        let doc = self.name()?;
        let user_flags = self.u32()?;
        let flags = self.u8()?;
        Ok(Function {
            returns,
            doc,
            user_flags,
            global: flags & 1 != 0,
            native: flags & 2 != 0,
            parameters: self.counted(Self::pair)?,
            locals: self.counted(Self::pair)?,
            code: self.counted(|r| r.instruction(game))?,
        })
    }

    fn instruction(&mut self, game: u16) -> Result<Instruction, Error> {
        let at = self.at;
        let code = self.u8()?;
        let Some(op) = Op::of(code, game) else {
            return self.fail(at, format!("unknown opcode {code:#x}"));
        };
        let args = self.list(usize::from(op.args), Self::value)?;
        let mut rest = Vec::new();
        if op.variadic {
            let at = self.at;
            let Value::Integer(count) = self.value()? else {
                return self.fail(at, "a count of arguments should be an integer");
            };
            let Ok(count) = usize::try_from(count) else {
                return self.fail(at, format!("{count} arguments"));
            };
            rest = self.list(count, Self::value)?;
        }
        Ok(Instruction { op, args, rest })
    }
}

impl Pex {
    /// Reads a file.
    pub fn read(bytes: &[u8]) -> Result<Self, Error> {
        let mut reader = Reader {
            bytes,
            at: 0,
            big_endian: false,
            strings: 0,
        };
        reader.big_endian = match reader.take(4)? {
            [0xFA, 0x57, 0xC0, 0xDE] => true,
            [0xDE, 0xC0, 0x57, 0xFA] => false,
            _ => return reader.fail(0, "not a .pex file"),
        };
        let major = reader.u8()?;
        let minor = reader.u8()?;
        if major != 3 {
            return reader.fail(4, format!("unknown version {major}.{minor}"));
        }
        let at = reader.at;
        let game = reader.u16()?;
        if !matches!(game, SKYRIM | FALLOUT4 | FALLOUT76 | STARFIELD) {
            return reader.fail(at, format!("unknown game {game}"));
        }
        let compile_time = reader.u64()?;
        let source = reader.string()?;
        let user = reader.string()?;
        let computer = reader.string()?;
        let strings = reader.counted(Reader::string)?;
        reader.strings = strings.len();
        let debug = reader.bool()?.then(|| reader.debug(game)).transpose()?;
        let user_flags = reader.counted(|r| {
            Ok(UserFlag {
                name: r.name()?,
                bit: r.u8()?,
            })
        })?;
        let objects = reader.counted(|r| r.object(game))?;
        Ok(Self {
            major,
            minor,
            game,
            compile_time,
            source,
            user,
            computer,
            strings,
            debug,
            user_flags,
            objects,
        })
    }
}
