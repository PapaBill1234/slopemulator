//! Generator for the checked-in files in `src/generated/`.
//!
//! The generator reads the client RE tables (TSV) and writes Rust source text.
//! It does not write files. The `gen-ids` bin and the test `generated_is_current` use it.

use std::collections::{BTreeMap, HashMap};
use std::fmt::{self, Write as _};
use std::path::{Path, PathBuf};

/// Directory with the TSV files, relative to the workspace root.
const RE_DIR: &str = "Docs/Client/RE";

/// One command table: Rust enum name and TSV file name.
const COMMAND_TABLES: [(&str, &str); 4] = [
    ("GameCmd", "game-commands.tsv"),
    ("LobbyCmd", "lobby-commands.tsv"),
    ("StsCmd", "03-sts-commands.tsv"),
    ("InfoGateCmd", "12-infogate-commands.tsv"),
];

/// Rust keywords. A name that is a keyword cannot be a const or a module name.
const KEYWORDS: [&str; 54] = [
    "as", "async", "await", "break", "const", "continue", "crate", "dyn", "else", "enum", "extern",
    "false", "fn", "for", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub",
    "ref", "return", "self", "Self", "static", "struct", "super", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro",
    "override", "priv", "typeof", "unsized", "virtual", "yield", "try", "gen", "_", "union",
];

/// Error from the generator.
#[derive(Debug)]
pub enum GenError {
    /// A file cannot be read.
    Io { path: PathBuf, message: String },
    /// A table has no header, a missing column or a bad cell.
    Parse {
        file: String,
        line: usize,
        message: String,
    },
    /// Two rows of one table or enum have the same name.
    DuplicateName {
        table: String,
        name: String,
        first_line: usize,
        line: usize,
    },
    /// Two rows of one command table have the same ID.
    DuplicateId {
        table: String,
        id: i64,
        first_line: usize,
        line: usize,
    },
    /// A name cannot be an identifier.
    BadName {
        file: String,
        line: usize,
        name: String,
    },
}

impl fmt::Display for GenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, message } => write!(f, "{}: {message}", path.display()),
            Self::Parse {
                file,
                line,
                message,
            } => write!(f, "{file}:{line}: {message}"),
            Self::DuplicateName {
                table,
                name,
                first_line,
                line,
            } => write!(
                f,
                "duplicate name {name:?} in {table}: line {first_line} and line {line}"
            ),
            Self::DuplicateId {
                table,
                id,
                first_line,
                line,
            } => write!(
                f,
                "duplicate ID {id} in {table}: line {first_line} and line {line}"
            ),
            Self::BadName { file, line, name } => {
                write!(f, "{file}:{line}: name {name:?} is not an identifier")
            }
        }
    }
}

impl std::error::Error for GenError {}

/// Keep letters, digits and `_` (Rust allows Unicode letters in a name, for example Korean
/// enum names). Replace each other character with `_`.
pub fn sanitize(text: &str) -> String {
    text.trim()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// True if the text can be a Rust identifier (and is not a keyword).
pub fn is_valid_ident(text: &str) -> bool {
    match text.chars().next() {
        None => false,
        Some(c) if c.is_numeric() => false,
        Some(_) => !KEYWORDS.contains(&text),
    }
}

/// A TSV file: header names and data rows with their 1-based line numbers.
struct Tsv {
    file: String,
    header: Vec<String>,
    rows: Vec<(usize, Vec<String>)>,
}

impl Tsv {
    fn read(root: &Path, file: &str) -> Result<Self, GenError> {
        let path = root.join(RE_DIR).join(file);
        let text = std::fs::read_to_string(&path).map_err(|e| GenError::Io {
            path: path.clone(),
            message: e.to_string(),
        })?;
        let mut lines = text.lines().enumerate();
        let header = match lines.next() {
            Some((_, h)) => h.split('\t').map(|c| c.trim().to_string()).collect(),
            None => {
                return Err(GenError::Parse {
                    file: file.to_string(),
                    line: 1,
                    message: "empty file".to_string(),
                })
            }
        };
        let mut rows = Vec::new();
        for (index, line) in lines {
            if line.trim().is_empty() {
                continue;
            }
            rows.push((index + 1, line.split('\t').map(|c| c.to_string()).collect()));
        }
        Ok(Self {
            file: file.to_string(),
            header,
            rows,
        })
    }

    /// Index of the first header that matches one of the names.
    fn column(&self, names: &[&str]) -> Result<usize, GenError> {
        for name in names {
            if let Some(i) = self.header.iter().position(|h| h == name) {
                return Ok(i);
            }
        }
        Err(GenError::Parse {
            file: self.file.clone(),
            line: 1,
            message: format!("missing column {}", names.join(" or ")),
        })
    }

    fn cell(row: &[String], column: usize) -> &str {
        row.get(column).map(|c| c.trim()).unwrap_or("")
    }

    fn number(&self, line: usize, text: &str) -> Result<i64, GenError> {
        let bad = || GenError::Parse {
            file: self.file.clone(),
            line,
            message: format!("bad number {text:?}"),
        };
        let (negative, digits) = match text.strip_prefix('-') {
            Some(rest) => (true, rest),
            None => (false, text),
        };
        // A value above `i64::MAX` is read as `u64`. The const keeps the same bits as `i64`
        // (so `18446744073709551615` becomes `-1`).
        let magnitude = match digits
            .strip_prefix("0x")
            .or_else(|| digits.strip_prefix("0X"))
        {
            Some(hex) => u64::from_str_radix(hex, 16).map_err(|_| bad())?,
            None => digits.parse::<u64>().map_err(|_| bad())?,
        };
        let value = magnitude as i64;
        Ok(if negative {
            value.wrapping_neg()
        } else {
            value
        })
    }
}

/// One row of a command table.
struct Command {
    id: i64,
    name: String,
    ident: String,
    direction: String,
}

fn read_commands(root: &Path, enum_name: &str, file: &str) -> Result<Vec<Command>, GenError> {
    let tsv = Tsv::read(root, file)?;
    let id_col = tsv.column(&["id"])?;
    let name_col = tsv.column(&["name"])?;
    let dir_col = tsv.column(&["direction"])?;
    let mut out: Vec<Command> = Vec::new();
    let mut by_name: HashMap<String, usize> = HashMap::new();
    let mut by_id: HashMap<i64, usize> = HashMap::new();
    for (line, row) in &tsv.rows {
        let name = Tsv::cell(row, name_col).to_string();
        let id = tsv.number(*line, Tsv::cell(row, id_col))?;
        let ident = sanitize(&name);
        if !is_valid_ident(&ident) {
            return Err(GenError::BadName {
                file: file.to_string(),
                line: *line,
                name,
            });
        }
        if let Some(first) = by_name.get(&ident) {
            return Err(GenError::DuplicateName {
                table: enum_name.to_string(),
                name,
                first_line: *first,
                line: *line,
            });
        }
        if let Some(first) = by_id.get(&id) {
            return Err(GenError::DuplicateId {
                table: enum_name.to_string(),
                id,
                first_line: *first,
                line: *line,
            });
        }
        by_name.insert(ident.clone(), *line);
        by_id.insert(id, *line);
        out.push(Command {
            id,
            name,
            ident,
            direction: Tsv::cell(row, dir_col).to_string(),
        });
    }
    Ok(out)
}

/// LobbyDaemon command tables: table name part (between `?` and `PacketLobbySideHandlerList`),
/// Rust enum name.
const LOBBYD_TABLES: [(&str, &str); 3] = [
    ("LobbyLobbyGate", "LobbyGateLobbydCmd"),
    ("LobbyGame", "GameLobbydCmd"),
    ("AccountInventoryLobby", "AccountInventoryLobbydCmd"),
];

/// Path of the LobbyDaemon command tables, relative to the workspace root.
const LOBBYD_JSON: &str = "Docs/Server/LobbyDaemon/command-tables.json";

/// Mark of a verified name in `name_status`.
const VERIFIED: &str = "\u{2705}";

fn lobbyd_error(message: impl Into<String>) -> GenError {
    GenError::Parse {
        file: LOBBYD_JSON.to_string(),
        line: 0,
        message: message.into(),
    }
}

/// Read the rows with a verified name (✅) of one LobbyDaemon handler table.
fn read_lobbyd_commands(
    table: &serde_json::Value,
    table_key: &str,
    enum_name: &str,
) -> Result<Vec<Command>, GenError> {
    let rows = table
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or_else(|| lobbyd_error(format!("table {table_key}: no rows")))?;
    let mut out: Vec<Command> = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        if row.get("name_status").and_then(|v| v.as_str()) != Some(VERIFIED) {
            continue;
        }
        let id = row
            .get("command")
            .and_then(|v| v.as_i64())
            .ok_or_else(|| lobbyd_error(format!("table {table_key}: row {index}: no command")))?;
        let full = row
            .get("name")
            .and_then(|v| v.as_str())
            .ok_or_else(|| lobbyd_error(format!("table {table_key}: row {index}: no name")))?;
        // `HandleLobbyLobbyGate_StartSessionWithNPTokenReq` -> `StartSessionWithNPTokenReq`.
        let name = full
            .strip_prefix("Handle")
            .and_then(|rest| rest.split_once('_'))
            .map(|(_, short)| short)
            .ok_or_else(|| lobbyd_error(format!("table {table_key}: bad name {full:?}")))?;
        let ident = sanitize(name);
        if !is_valid_ident(&ident) {
            return Err(GenError::BadName {
                file: LOBBYD_JSON.to_string(),
                line: index,
                name: full.to_string(),
            });
        }
        if let Some(first) = out.iter().position(|c| c.ident == ident) {
            return Err(GenError::DuplicateName {
                table: enum_name.to_string(),
                name: full.to_string(),
                first_line: first,
                line: index,
            });
        }
        if let Some(first) = out.iter().position(|c| c.id == id) {
            return Err(GenError::DuplicateId {
                table: enum_name.to_string(),
                id,
                first_line: first,
                line: index,
            });
        }
        out.push(Command {
            id,
            name: name.to_string(),
            ident,
            direction: table_key.to_string(),
        });
    }
    Ok(out)
}

/// Make the text of `lobbyd.rs`: one enum for each LobbyDaemon handler table. The enums have
/// only the rows with a verified name.
pub fn generate_lobbyd(root: &Path) -> Result<String, GenError> {
    let path = root.join(LOBBYD_JSON);
    let text = std::fs::read_to_string(&path).map_err(|e| GenError::Io {
        path: path.clone(),
        message: e.to_string(),
    })?;
    let json: serde_json::Value =
        serde_json::from_str(&text).map_err(|e| lobbyd_error(e.to_string()))?;
    let tables = json
        .get("tables")
        .and_then(|v| v.as_array())
        .ok_or_else(|| lobbyd_error("no tables"))?;
    let mut out = String::new();
    out.push_str("// @generated by `cargo run -p packet-ids --bin gen-ids`. Do not edit.\n");
    let _ = writeln!(
        out,
        "// Source: {LOBBYD_JSON} (rows with a verified name only)\n"
    );
    for (table_key, enum_name) in LOBBYD_TABLES {
        let wanted = format!("?{table_key}PacketLobbySideHandlerList");
        let table = tables
            .iter()
            .find(|t| {
                t.get("name")
                    .and_then(|v| v.as_str())
                    .is_some_and(|n| n.starts_with(&wanted))
            })
            .ok_or_else(|| lobbyd_error(format!("table {table_key} not found")))?;
        let rows = read_lobbyd_commands(table, table_key, enum_name)?;
        let doc = format!("LobbyDaemon commands of the table `{table_key}` (verified names only).");
        write_commands(&mut out, enum_name, &doc, &rows);
    }
    while out.ends_with("\n\n") {
        out.pop();
    }
    Ok(out)
}

fn write_commands(out: &mut String, enum_name: &str, doc: &str, commands: &[Command]) {
    let fits_i16 = commands
        .iter()
        .all(|c| c.id >= i64::from(i16::MIN) && c.id <= i64::from(i16::MAX));
    let repr = if fits_i16 { "i16" } else { "i32" };
    let _ = writeln!(out, "/// {doc}");
    let _ = writeln!(out, "#[allow(non_camel_case_types)]");
    let _ = writeln!(out, "#[repr({repr})]");
    let _ = writeln!(
        out,
        "#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]\npub enum {enum_name} {{"
    );
    for c in commands {
        let _ = writeln!(out, "    /// {} `{}`", c.direction, c.name);
        let _ = writeln!(out, "    {} = {},", c.ident, c.id);
    }
    let _ = writeln!(out, "}}\n");
    let _ = writeln!(out, "impl {enum_name} {{");
    let _ = writeln!(out, "    /// Number of commands in the table.");
    let _ = writeln!(out, "    pub const COUNT: usize = {};\n", commands.len());
    let _ = writeln!(out, "    /// Command ID on the wire.");
    let _ = writeln!(out, "    pub const fn id(self) -> {repr} {{");
    let _ = writeln!(out, "        self as {repr}");
    let _ = writeln!(out, "    }}\n");
    let _ = writeln!(
        out,
        "    /// Command for an ID, or `None` if the table has no such ID."
    );
    let _ = writeln!(out, "    pub fn from_id(id: {repr}) -> Option<Self> {{");
    let _ = writeln!(out, "        match id {{");
    for c in commands {
        let _ = writeln!(out, "            {} => Some(Self::{}),", c.id, c.ident);
    }
    let _ = writeln!(out, "            _ => None,");
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "    }}\n");
    let _ = writeln!(out, "    /// Official name, as in the TSV.");
    let _ = writeln!(out, "    pub fn name(self) -> &'static str {{");
    let _ = writeln!(out, "        match self {{");
    for c in commands {
        let _ = writeln!(out, "            Self::{} => {:?},", c.ident, c.name);
    }
    let _ = writeln!(out, "        }}");
    let _ = writeln!(out, "    }}");
    let _ = writeln!(out, "}}\n");
}

/// One enum value row.
struct EnumRow {
    ident: String,
    value: i64,
    file: String,
    line: usize,
}

/// All rows of one enum. `file` is the first file that has the enum.
struct EnumDef {
    original: String,
    rows: Vec<EnumRow>,
}

fn enum_files(root: &Path) -> Result<Vec<String>, GenError> {
    let dir = root.join(RE_DIR);
    let io = |e: std::io::Error| GenError::Io {
        path: dir.clone(),
        message: e.to_string(),
    };
    let mut files = Vec::new();
    for entry in std::fs::read_dir(&dir).map_err(io)? {
        let name = entry.map_err(io)?.file_name().to_string_lossy().to_string();
        if name.ends_with(".tsv") && name.contains("-enums") {
            files.push(name);
        }
    }
    files.sort();
    Ok(files)
}

/// Enums by module name, count of skipped rows, and the list of repeated rows.
type EnumTables = (BTreeMap<String, EnumDef>, usize, Vec<String>);

fn read_enums(root: &Path) -> Result<EnumTables, GenError> {
    let mut defs: BTreeMap<String, EnumDef> = BTreeMap::new();
    let mut skipped = 0usize;
    let mut repeats: Vec<String> = Vec::new();
    for file in enum_files(root)? {
        let tsv = Tsv::read(root, &file)?;
        let enum_col = tsv.column(&["enum", "enum_gm"])?;
        let name_col = tsv.column(&["name", "member"])?;
        let value_col = tsv.column(&["value"])?;
        for (line, row) in &tsv.rows {
            let original = Tsv::cell(row, enum_col);
            let module = sanitize(original);
            if !is_valid_ident(&module) {
                return Err(GenError::BadName {
                    file: file.clone(),
                    line: *line,
                    name: original.to_string(),
                });
            }
            let ident = sanitize(Tsv::cell(row, name_col));
            if !is_valid_ident(&ident) {
                skipped += 1;
                continue;
            }
            let value = tsv.number(*line, Tsv::cell(row, value_col))?;
            let def = defs.entry(module.clone()).or_insert_with(|| EnumDef {
                original: original.to_string(),
                rows: Vec::new(),
            });
            if def.original != original {
                return Err(GenError::DuplicateName {
                    table: module,
                    name: original.to_string(),
                    first_line: 0,
                    line: *line,
                });
            }
            if let Some(first) = def.rows.iter().find(|r| r.ident == ident) {
                // Same name and same value in two places: keep the first, write a comment.
                // Same name and other value: this is a real conflict, so stop.
                if first.value != value {
                    return Err(GenError::DuplicateName {
                        table: original.to_string(),
                        name: ident,
                        first_line: first.line,
                        line: *line,
                    });
                }
                repeats.push(format!(
                    "{original}::{ident} = {value}: kept {}:{}, dropped {file}:{line}",
                    first.file, first.line
                ));
                continue;
            }
            def.rows.push(EnumRow {
                ident,
                value,
                file: file.clone(),
                line: *line,
            });
        }
    }
    Ok((defs, skipped, repeats))
}

/// Make the text of `commands.rs` and `enums.rs`. `root` is the workspace root.
pub fn generate(root: &Path) -> Result<(String, String), GenError> {
    let mut commands = String::new();
    commands.push_str("// @generated by `cargo run -p packet-ids --bin gen-ids`. Do not edit.\n");
    commands.push_str("// Source: Docs/Client/RE/*-commands.tsv\n\n");
    for (enum_name, file) in COMMAND_TABLES {
        let rows = read_commands(root, enum_name, file)?;
        let doc = format!("Commands from `{RE_DIR}/{file}`.");
        write_commands(&mut commands, enum_name, &doc, &rows);
    }
    while commands.ends_with("\n\n") {
        commands.pop();
    }

    let (defs, skipped, repeats) = read_enums(root)?;
    let mut enums = String::new();
    enums.push_str("// @generated by `cargo run -p packet-ids --bin gen-ids`. Do not edit.\n");
    enums.push_str("// Source: Docs/Client/RE/*-enums*.tsv\n");
    let _ = writeln!(
        enums,
        "// Skipped rows (name is not a valid identifier): {skipped}"
    );
    let _ = writeln!(
        enums,
        "// Repeated rows (same enum, name and value; first row kept): {}",
        repeats.len()
    );
    for repeat in &repeats {
        let _ = writeln!(enums, "//   {repeat}");
    }
    enums.push_str("#![allow(non_snake_case, non_upper_case_globals)]\n");
    for (module, def) in &defs {
        let _ = writeln!(enums, "\n/// Enum `{}`.", def.original);
        let _ = writeln!(enums, "pub mod {module} {{");
        for row in &def.rows {
            let _ = writeln!(enums, "    pub const {}: i64 = {};", row.ident, row.value);
        }
        let _ = writeln!(enums, "}}");
    }
    Ok((commands, enums))
}
