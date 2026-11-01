//! The text of a script, in the assembly `PapyrusAssembler -D` writes.
//!
//! The same text but for two things: the tables are in the order of the file,
//! where it follows its own hash tables, and the dates in the comments of
//! `.info` are in UTC, not in the time zone of the machine.

use std::collections::{BTreeMap, HashMap};
use std::fmt::Write;

use crate::{Debug, Function, Instruction, Member, Name, Object, Pex, Property, Value, Variable};

/// Where the comment of an instruction starts, padding included.
const COMMENT_COLUMN: usize = 55;

const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// `Wed Jan 17 21:01:41 2024`, in UTC.
fn date(seconds: u64) -> String {
    let days = seconds / 86_400;
    let time = seconds % 86_400;
    // Days to a civil date, the year starting in March for leap days to come
    // last.
    let shifted = days + 719_468;
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let shifted_month = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * shifted_month + 2) / 5 + 1;
    let month = (shifted_month + 2) % 12;
    let year = year_of_era + era * 400 + u64::from(month < 2);
    format!(
        "{} {} {day:02} {:02}:{:02}:{:02} {year}",
        WEEKDAYS[(days % 7) as usize],
        MONTHS[month as usize],
        time / 3_600,
        time / 60 % 60,
        time % 60,
    )
}

/// Between quotes, escaped.
fn quoted(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

struct Writer<'a> {
    pex: &'a Pex,
    out: String,
    /// The next label: their numbers run through the file.
    label: usize,
    /// The lines of each function, by object, state, function and kind.
    lines: HashMap<(Name, Name, Name, u8), &'a [u16]>,
}

impl<'a> Writer<'a> {
    fn text(&self, name: Name) -> &'a str {
        self.pex.text(name)
    }

    fn line(&mut self, depth: usize, text: &str) {
        for _ in 0..depth {
            self.out.push('\t');
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn flags(&mut self, depth: usize, flags: u32) {
        self.line(
            depth,
            &format!(".userFlags {flags}\t; Flags: {flags:#010x}"),
        );
    }

    fn doc(&mut self, depth: usize, doc: Name) {
        let doc = quoted(self.text(doc));
        self.line(depth, &format!(".docString {doc}"));
    }

    fn value(&self, value: Value) -> String {
        match value {
            Value::None => "None".to_owned(),
            Value::Identifier(name) => self.text(name).to_owned(),
            Value::String(name) => quoted(self.text(name)),
            Value::Integer(value) => value.to_string(),
            Value::Float(value) => format!("{value:.6}"),
            Value::Bool(true) => "True".to_owned(),
            Value::Bool(false) => "False".to_owned(),
        }
    }

    fn info(&mut self) {
        let pex = self.pex;
        self.line(0, ".info");
        self.line(1, &format!(".source {}", quoted(&pex.source)));
        if let Some(Debug { modify_time, .. }) = pex.debug {
            let date = date(modify_time);
            self.line(1, &format!(".modifyTime {modify_time} ;{date} UTC"));
        }
        let date = date(pex.compile_time);
        self.line(1, &format!(".compileTime {} ;{date} UTC", pex.compile_time));
        self.line(1, &format!(".user {}", quoted(&pex.user)));
        self.line(1, &format!(".computer {}", quoted(&pex.computer)));
        self.line(0, ".endInfo");
        self.line(0, ".userFlagsRef");
        for flag in &pex.user_flags {
            let name = self.text(flag.name);
            let bit = flag.bit;
            self.line(1, &format!(".flag {name} {bit}\t; {bit:#010x}"));
        }
        self.line(0, ".endUserFlagsRef");
    }

    fn object(&mut self, object: &Object) {
        let game = self.pex.game;
        let name = self.text(object.name);
        let parent = self.text(object.parent);
        let constant = if object.constant { " const" } else { "" };
        self.line(1, &format!(".object {name} {parent}{constant}"));
        self.flags(2, object.user_flags);
        self.doc(2, object.doc);
        let auto_state = self.text(object.auto_state);
        self.line(2, &format!(".autoState {auto_state}"));
        if game > 1 {
            self.line(2, ".structTable");
            for structure in &object.structs {
                let name = self.text(structure.name);
                self.line(3, &format!(".struct {name}"));
                for member in &structure.members {
                    self.member(member);
                }
                self.line(3, ".endStruct");
            }
            self.line(2, ".endStructTable");
        }
        self.line(2, ".variableTable");
        for variable in &object.variables {
            self.variable(variable);
        }
        self.line(2, ".endVariableTable");
        if game == 4 {
            self.line(2, ".guardTable");
            for &guard in &object.guards {
                let guard = self.text(guard);
                self.line(3, &format!(".guard {guard}"));
            }
            self.line(2, ".endGuardTable");
        }
        self.line(2, ".propertyTable");
        for property in &object.properties {
            self.property(object, property);
        }
        self.line(2, ".endPropertyTable");
        if game > 1 {
            self.property_groups(object);
        }
        self.line(2, ".stateTable");
        for state in &object.states {
            let name = self.text(state.name);
            self.line(3, &format!(".state {name}"));
            for (name, function) in &state.functions {
                let key = (object.name, state.name, *name, 0);
                self.function(4, self.text(*name), function, key);
            }
            self.line(3, ".endState");
        }
        self.line(2, ".endStateTable");
        self.line(1, ".endObject");
    }

    fn member(&mut self, member: &Member) {
        let name = self.text(member.name);
        let ty = self.text(member.ty);
        let constant = if member.constant { " const" } else { "" };
        self.line(4, &format!(".variable {name} {ty}{constant}"));
        self.flags(5, member.user_flags);
        let value = self.value(member.value);
        self.line(5, &format!(".initialValue {value}"));
        self.doc(5, member.doc);
        self.line(4, ".endVariable");
    }

    fn variable(&mut self, variable: &Variable) {
        let name = self.text(variable.name);
        let ty = self.text(variable.ty);
        let constant = if variable.constant { " const" } else { "" };
        self.line(3, &format!(".variable {name} {ty}{constant}"));
        self.flags(4, variable.user_flags);
        let value = self.value(variable.value);
        self.line(4, &format!(".initialValue {value}"));
        self.line(3, ".endVariable");
    }

    fn property(&mut self, object: &Object, property: &Property) {
        let name = self.text(property.name);
        let ty = self.text(property.ty);
        let auto = if property.auto_var.is_some() {
            "auto"
        } else {
            ""
        };
        self.line(3, &format!(".property {name} {ty} {auto}"));
        self.flags(4, property.user_flags);
        self.doc(4, property.doc);
        if let Some(auto_var) = property.auto_var {
            let auto_var = self.text(auto_var);
            self.line(4, &format!(".autoVar {auto_var}"));
        }
        let state = self.empty();
        if let Some(get) = &property.get {
            self.function(4, "get", get, (object.name, state, property.name, 1));
        }
        if let Some(set) = &property.set {
            self.function(4, "set", set, (object.name, state, property.name, 2));
        }
        self.line(3, ".endProperty");
    }

    /// The default state's name, for the lines of an accessor.
    fn empty(&self) -> Name {
        let found = self.pex.strings.iter().position(String::is_empty);
        found.map_or(Name::MAX, |index| index as Name)
    }

    fn property_groups(&mut self, object: &Object) {
        self.line(2, ".propertyGroupTable");
        let pex = self.pex;
        let groups = pex.debug.iter().flat_map(|debug| &debug.property_groups);
        for group in groups.filter(|group| group.object == object.name) {
            let name = self.text(group.name);
            self.line(3, &format!(".propertyGroup {name}"));
            self.flags(4, group.user_flags);
            self.doc(4, group.doc);
            for &property in &group.properties {
                let property = self.text(property);
                self.line(4, &format!(".property {property}"));
            }
            self.line(3, ".endPropertyGroup");
        }
        self.line(2, ".endPropertyGroupTable");
    }

    fn function(
        &mut self,
        depth: usize,
        name: &str,
        function: &Function,
        key: (Name, Name, Name, u8),
    ) {
        let native = if function.native { " native" } else { "" };
        let global = if function.global { " static" } else { "" };
        self.line(depth, &format!(".function {name}{native}{global}"));
        self.flags(depth + 1, function.user_flags);
        self.doc(depth + 1, function.doc);
        let returns = self.text(function.returns);
        self.line(depth + 1, &format!(".return {returns}"));
        self.line(depth + 1, ".paramTable");
        for &(name, ty) in &function.parameters {
            let line = format!(".param {} {}", self.text(name), self.text(ty));
            self.line(depth + 2, &line);
        }
        self.line(depth + 1, ".endParamTable");
        if !function.native {
            self.line(depth + 1, ".localTable");
            for &(name, ty) in &function.locals {
                let line = format!(".local {} {}", self.text(name), self.text(ty));
                self.line(depth + 2, &line);
            }
            self.line(depth + 1, ".endLocalTable");
            self.line(depth + 1, ".code");
            self.code(depth + 2, &function.code, self.lines.get(&key).copied());
            self.line(depth + 1, ".endCode");
        }
        self.line(depth, ".endFunction");
    }

    fn code(&mut self, depth: usize, code: &[Instruction], lines: Option<&[u16]>) {
        let labels = self.labels(code);
        for (index, instruction) in code.iter().enumerate() {
            if let Some(label) = labels.get(&index) {
                self.line(depth - 1, &format!("_label{label}:"));
            }
            let mut text = instruction.op.name.to_owned();
            let jumps = instruction.op.jumps();
            for (position, &arg) in instruction.args.iter().enumerate() {
                text.push(' ');
                match arg {
                    Value::Integer(offset) if jumps && position + 1 == instruction.args.len() => {
                        let label = target(index, offset).and_then(|target| labels.get(&target));
                        match label {
                            Some(label) => {
                                let _ = write!(text, "_label{label}");
                            }
                            None => text.push_str(&offset.to_string()),
                        }
                    }
                    arg => text.push_str(&self.value(arg)),
                }
            }
            for &arg in &instruction.rest {
                text.push(' ');
                text.push_str(&self.value(arg));
            }
            // Starfield's assembler no longer ends each instruction with a
            // space, which only shows past the column.
            let width = if self.pex.game == 4 {
                COMMENT_COLUMN
            } else {
                text.push(' ');
                COMMENT_COLUMN + 1
            };
            // Compilers leave some instructions out of the array.
            let line = match lines {
                None => "??".to_owned(),
                Some(lines) => match (lines.get(index), lines.last()) {
                    (Some(line), _) => line.to_string(),
                    (None, Some(last)) => format!("{last} - past end of debug array"),
                    (None, None) => "??".to_owned(),
                },
            };
            let line = format!("{text:<width$} ;@line {line}");
            self.line(depth, &line);
        }
        if let Some(label) = labels.get(&code.len()) {
            self.line(depth - 1, &format!("_label{label}:"));
        }
    }

    /// The label of each position a jump of `code` leads to, in the order of
    /// the jumps.
    fn labels(&mut self, code: &[Instruction]) -> BTreeMap<usize, usize> {
        let mut labels = BTreeMap::new();
        for (index, instruction) in code.iter().enumerate() {
            if !instruction.op.jumps() {
                continue;
            }
            let Some(&Value::Integer(offset)) = instruction.args.last() else {
                continue;
            };
            let Some(target) = target(index, offset).filter(|&target| target <= code.len()) else {
                continue;
            };
            labels.entry(target).or_insert_with(|| {
                self.label += 1;
                self.label - 1
            });
        }
        labels
    }
}

/// Where a jump from `index` leads.
fn target(index: usize, offset: i32) -> Option<usize> {
    index.checked_add_signed(offset as isize)
}

/// The text of `pex`.
pub fn disassemble(pex: &Pex) -> String {
    let functions = pex.debug.iter().flat_map(|debug| &debug.functions);
    let lines = functions
        .map(|f| ((f.object, f.state, f.function, f.kind), f.lines.as_slice()))
        .collect();
    let mut writer = Writer {
        pex,
        out: String::new(),
        label: 0,
        lines,
    };
    writer.info();
    writer.line(0, ".objectTable");
    for object in &pex.objects {
        writer.object(object);
    }
    writer.line(0, ".endObjectTable");
    writer.out
}

#[cfg(test)]
mod tests {
    use super::date;

    #[test]
    fn dates_are_those_of_the_calendar() {
        assert_eq!(date(0), "Thu Jan 01 00:00:00 1970");
        assert_eq!(date(1_705_521_701), "Wed Jan 17 20:01:41 2024");
        assert_eq!(date(1_709_164_800), "Thu Feb 29 00:00:00 2024");
    }
}
