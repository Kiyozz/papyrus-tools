//! Declared types.
//!
//! Only what declarations say: the type of a variable, a property, a
//! parameter, the return type of a function, the target of a cast. Whatever
//! cannot be known that way is [`Ty::Unknown`], never an error.

use std::fmt;

/// The type of a value.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Ty {
    /// Not known from the declarations.
    Unknown,
    /// The `None` literal.
    None,
    /// `bool`
    Bool,
    /// `int`
    Int,
    /// `float`
    Float,
    /// `string`
    String,
    /// `var`, Fallout 4 onwards.
    Var,
    /// A script or a struct, as written: `Actor`, `MyMod:Quest`, `Entry`.
    Object(String),
    /// An array.
    Array(Box<Ty>),
}

impl Ty {
    /// The type written `written`: `int`, `Actor[]`, `MyMod:Base`.
    pub fn parse(written: &str) -> Ty {
        let written = written.trim();
        if let Some(element) = written.strip_suffix("[]") {
            return Ty::Array(Box::new(Ty::parse(element)));
        }
        match written.to_ascii_lowercase().as_str() {
            "" => Ty::Unknown,
            "bool" => Ty::Bool,
            "int" => Ty::Int,
            "float" => Ty::Float,
            "string" => Ty::String,
            "var" => Ty::Var,
            _ => Ty::Object(written.to_owned()),
        }
    }

    /// The script or struct name, for an object.
    pub fn object(&self) -> Option<&str> {
        match self {
            Ty::Object(name) => Some(name),
            _ => None,
        }
    }
}

impl fmt::Display for Ty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Ty::Unknown => f.write_str("?"),
            Ty::None => f.write_str("None"),
            Ty::Bool => f.write_str("bool"),
            Ty::Int => f.write_str("int"),
            Ty::Float => f.write_str("float"),
            Ty::String => f.write_str("string"),
            Ty::Var => f.write_str("var"),
            Ty::Object(name) => f.write_str(name),
            Ty::Array(element) => write!(f, "{element}[]"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn written_types_are_read() {
        assert_eq!(Ty::parse("INT"), Ty::Int);
        assert_eq!(
            Ty::parse("Actor[]"),
            Ty::Array(Box::new(Ty::Object("Actor".into())))
        );
        assert_eq!(Ty::parse("MyMod:Base").to_string(), "MyMod:Base");
        assert_eq!(Ty::parse(""), Ty::Unknown);
    }
}
