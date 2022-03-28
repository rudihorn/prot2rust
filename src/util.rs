use std::borrow::Cow;

use inflections::Inflect;
use proc_macro2::{Ident, Literal, Span, TokenStream};
use quote::ToTokens;
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Result};

pub const BITS_PER_BYTE: u32 = 8;

/// List of chars that some vendors use in their peripheral/field names but
/// that are not valid in Rust ident
const BLACKLIST_CHARS: &[char] = &['(', ')', '[', ']', '/', ' ', '-'];

#[derive(Clone, PartialEq, Debug)]
pub struct Config {
    pub target: Target,
    pub nightly: bool,
    pub generic_mod: bool,
    pub make_mod: bool,
    pub const_generic: bool,
    pub ignore_groups: bool,
    pub keep_list: bool,
    pub strict: bool,
    pub output_dir: PathBuf,
    pub source_type: SourceType,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            target: Target::default(),
            nightly: false,
            generic_mod: false,
            make_mod: false,
            const_generic: false,
            ignore_groups: false,
            keep_list: false,
            strict: false,
            output_dir: PathBuf::from("."),
            source_type: SourceType::default(),
        }
    }
}

#[allow(clippy::upper_case_acronyms)]
#[allow(non_camel_case_types)]
#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Target {
    CortexM,
    Msp430,
    RISCV,
    XtensaLX,
    Mips,
    None,
}

impl Target {
    pub fn parse(s: &str) -> Result<Self> {
        Ok(match s {
            "cortex-m" => Target::CortexM,
            "msp430" => Target::Msp430,
            "riscv" => Target::RISCV,
            "xtensa-lx" => Target::XtensaLX,
            "mips" => Target::Mips,
            "none" => Target::None,
            _ => bail!("unknown target {}", s),
        })
    }
}

impl Default for Target {
    fn default() -> Self {
        Self::CortexM
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum SourceType {
    Xml,
    Yaml,
    Json,
}

impl Default for SourceType {
    fn default() -> Self {
        Self::Xml
    }
}

impl SourceType {
    /// Make a new [`Source`] from a given extension.
    pub fn from_extension(s: &str) -> Option<Self> {
        match s {
            "yml" | "yaml" => Some(Self::Yaml),
            "json" => Some(Self::Json),
            "svd" | "xml" => Some(Self::Xml),
            _ => None,
        }
    }
    pub fn from_path(path: &Path) -> Self {
        path.extension()
            .and_then(|e| e.to_str())
            .and_then(Self::from_extension)
            .unwrap_or_default()
    }
}

pub trait ToSanitizedPascalCase {
    fn to_sanitized_pascal_case(&self) -> Cow<str>;
}

pub trait ToSanitizedUpperCase {
    fn to_sanitized_upper_case(&self) -> Cow<str>;
}

pub trait ToSanitizedSnakeCase {
    fn to_sanitized_not_keyword_snake_case(&self) -> Cow<str>;
    fn to_sanitized_snake_case(&self) -> Cow<str> {
        let s = self.to_sanitized_not_keyword_snake_case();
        sanitize_keyword(s)
    }
}

impl ToSanitizedSnakeCase for str {
    fn to_sanitized_not_keyword_snake_case(&self) -> Cow<str> {
        const INTERNALS: [&str; 4] = ["set_bit", "clear_bit", "bit", "bits"];

        let s = self.replace(BLACKLIST_CHARS, "");
        match s.chars().next().unwrap_or('\0') {
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                format!("_{}", s.to_snake_case()).into()
            }
            _ => {
                let s = Cow::from(s.to_snake_case());
                if INTERNALS.contains(&s.as_ref()) {
                    s + "_"
                } else {
                    s
                }
            }
        }
    }
}

pub fn sanitize_keyword(sc: Cow<str>) -> Cow<str> {
    const KEYWORDS: [&str; 54] = [
        "abstract", "alignof", "as", "async", "await", "become", "box", "break", "const",
        "continue", "crate", "do", "else", "enum", "extern", "false", "final", "fn", "for", "if",
        "impl", "in", "let", "loop", "macro", "match", "mod", "move", "mut", "offsetof",
        "override", "priv", "proc", "pub", "pure", "ref", "return", "self", "sizeof", "static",
        "struct", "super", "trait", "true", "try", "type", "typeof", "unsafe", "unsized", "use",
        "virtual", "where", "while", "yield",
    ];
    if KEYWORDS.contains(&sc.as_ref()) {
        sc + "_"
    } else {
        sc
    }
}

impl ToSanitizedUpperCase for str {
    fn to_sanitized_upper_case(&self) -> Cow<str> {
        let s = self.replace(BLACKLIST_CHARS, "");

        match s.chars().next().unwrap_or('\0') {
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                Cow::from(format!("_{}", s.to_upper_case()))
            }
            _ => Cow::from(s.to_upper_case()),
        }
    }
}

impl ToSanitizedPascalCase for str {
    fn to_sanitized_pascal_case(&self) -> Cow<str> {
        let s = self.replace(BLACKLIST_CHARS, "");

        match s.chars().next().unwrap_or('\0') {
            '0' | '1' | '2' | '3' | '4' | '5' | '6' | '7' | '8' | '9' => {
                Cow::from(format!("_{}", s.to_pascal_case()))
            }
            _ => Cow::from(s.to_pascal_case()),
        }
    }
}

pub fn respace(s: &str) -> String {
    s.split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .replace(r"\n", "\n")
}

pub fn escape_brackets(s: &str) -> String {
    s.split('[')
        .fold("".to_string(), |acc, x| {
            if acc.is_empty() {
                x.to_string()
            } else if acc.ends_with('\\') {
                acc + "[" + x
            } else {
                acc + "\\[" + x
            }
        })
        .split(']')
        .fold("".to_string(), |acc, x| {
            if acc.is_empty() {
                x.to_string()
            } else if acc.ends_with('\\') {
                acc + "]" + x
            } else {
                acc + "\\]" + x
            }
        })
}

pub fn replace_suffix(name: &str, suffix: &str) -> String {
    if name.contains("[%s]") {
        name.replace("[%s]", suffix)
    } else {
        name.replace("%s", suffix)
    }
}
/// Turns `n` into an unsuffixed separated hex token
pub fn hex(n: u64) -> TokenStream {
    let (h4, h3, h2, h1) = (
        (n >> 48) & 0xffff,
        (n >> 32) & 0xffff,
        (n >> 16) & 0xffff,
        n & 0xffff,
    );
    syn::parse_str::<syn::Lit>(
        &(if h4 != 0 {
            format!("0x{:04x}_{:04x}_{:04x}_{:04x}", h4, h3, h2, h1)
        } else if h3 != 0 {
            format!("0x{:04x}_{:04x}_{:04x}", h3, h2, h1)
        } else if h2 != 0 {
            format!("0x{:04x}_{:04x}", h2, h1)
        } else if h1 & 0xff00 != 0 {
            format!("0x{:04x}", h1)
        } else if h1 != 0 {
            format!("0x{:02x}", h1 & 0xff)
        } else {
            "0".to_string()
        }),
    )
    .unwrap()
    .into_token_stream()
}

/// Turns `n` into an unsuffixed token
pub fn unsuffixed(n: u64) -> TokenStream {
    Literal::u64_unsuffixed(n).into_token_stream()
}

pub fn unsuffixed_or_bool(n: u64, width: u32) -> TokenStream {
    if width == 1 {
        Ident::new(if n == 0 { "false" } else { "true" }, Span::call_site()).into_token_stream()
    } else {
        unsuffixed(n)
    }
}

pub trait U32Ext {
    fn to_ty(&self) -> Result<Ident>;
    fn to_ty_width(&self) -> Result<u32>;
}

impl U32Ext for u32 {
    fn to_ty(&self) -> Result<Ident> {
        Ok(Ident::new(
            match *self {
                1 => "bool",
                2..=8 => "u8",
                9..=16 => "u16",
                17..=32 => "u32",
                33..=64 => "u64",
                _ => {
                    return Err(anyhow!(
                        "can't convert {} bits into a Rust integral type",
                        *self
                    ))
                }
            },
            Span::call_site(),
        ))
    }

    fn to_ty_width(&self) -> Result<u32> {
        Ok(match *self {
            1 => 1,
            2..=8 => 8,
            9..=16 => 16,
            17..=32 => 32,
            33..=64 => 64,
            _ => {
                return Err(anyhow!(
                    "can't convert {} bits into a Rust integral type width",
                    *self
                ))
            }
        })
    }
}
