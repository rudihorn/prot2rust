use anyhow::Result;

use std::collections::HashMap;

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::util::{ToSanitizedPascalCase, ToSanitizedSnakeCase, U32Ext};

pub struct PrimitiveMember {
    pub name: String,
    pub bytes: u32,
}

impl PrimitiveMember {
    pub fn new(name: &str, bytes: u32) -> Self {
        let name = String::from(name);
        Self { name, bytes }
    }
}

pub struct AlternativesMember {
    pub name: String,
    pub alternatives: String,
}

impl AlternativesMember {
    pub fn new(name: &str, alternatives: &str) -> Self {
        let name = String::from(name);
        let alternatives = String::from(alternatives);
        Self { name, alternatives }
    }
}

pub enum StructMember {
    PrimitiveMember(PrimitiveMember),
    AlternativesMember(AlternativesMember),
}

pub struct AlternativeOptions {
    pub name: String,
    pub default: String,
    pub alternatives: Vec<String>,
}

pub struct Alternatives {
    pub map: HashMap<String, AlternativeOptions>,
}

impl Alternatives {
    pub fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    pub fn insert(mut self, options: AlternativeOptions) -> Self {
        let key = options.name.clone();
        self.map.insert(key, options);
        self
    }

    pub fn insert_new_option<F>(mut self, key: &str, default: &Structure, mut f: F) -> Self
    where
        F: FnMut(AlternativeOptions) -> AlternativeOptions,
    {
        let options = AlternativeOptions::new(key, default);
        let options = f(options);
        let key = String::from(key);
        self.map.insert(key, options);
        self
    }

    pub fn get(&self, name: &str) -> Result<&AlternativeOptions> {
        let v = self.map.get(&String::from(name));
        match v {
            None => Err(anyhow::Error::msg("Could not find alternative.")),
            Some(v) => Ok(v),
        }
    }
}

pub struct Structure {
    pub name: String,
    pub members: Vec<StructMember>,
}

impl Structure {
    pub fn new(name: &str) -> Structure {
        let name = String::from(name);
        Structure {
            name,
            members: vec![],
        }
    }

    pub fn add_prim_field(mut self, name: &str, bytes: u32) -> Self {
        let member = PrimitiveMember::new(name, bytes);
        self.members.push(StructMember::PrimitiveMember(member));
        self
    }

    pub fn add_u8_field(self, name: &str) -> Self {
        self.add_prim_field(name, 1)
    }

    pub fn add_u16_field(self, name: &str) -> Self {
        self.add_prim_field(name, 2)
    }

    pub fn add_u32_field(self, name: &str) -> Self {
        self.add_prim_field(name, 4)
    }

    pub fn add_u64_field(self, name: &str) -> Self {
        self.add_prim_field(name, 4)
    }

    pub fn add_alt_field(mut self, name: &str, alternatives: &AlternativeOptions) -> Self {
        let member = AlternativesMember::new(name, &alternatives.name);
        self.members.push(StructMember::AlternativesMember(member));
        self
    }
}

impl AlternativeOptions {
    pub fn new(name: &str, default: &Structure) -> Self {
        let name = String::from(name);
        let default_name = default.name.clone();
        Self {
            name,
            default: default_name,
            alternatives: vec![],
        }
        .insert_struct(default)
    }

    pub fn insert_struct(mut self, structure: &Structure) -> Self {
        let name = structure.name.clone();
        self.alternatives.push(name);
        self
    }
}

pub fn render(structures: &Vec<Structure>, alternatives: &Alternatives) -> Result<TokenStream> {
    let span = Span::call_site();

    let mut mod_items = TokenStream::new();

    let mut trait_extends = TokenStream::new();

    for (key, alt) in &alternatives.map {
        let alt_pc = Ident::new(&key.to_sanitized_pascal_case(), span);
        let alt_pc_a = Ident::new(&format!("{}A", alt_pc), span);

        let mut alt_enum_entries = TokenStream::new();

        for altopt in &alt.alternatives {
            let alt_struct = Ident::new(&altopt.to_sanitized_pascal_case(), span);
            let alt_enum = Ident::new(&altopt.to_sanitized_pascal_case(), span);

            trait_extends.extend(quote! {
                impl #alt_pc for #alt_struct {
                    fn default() -> Self {
                        Self::new()
                    }
                }
            });
            alt_enum_entries.extend(quote! {
                #alt_enum,
            });
        }

        mod_items.extend(quote! {
            pub trait #alt_pc {
                fn default() -> Self;
            }

            enum #alt_pc_a {
                #alt_enum_entries
            }
        });
    }

    for structure in structures {
        let str_name = Ident::new(&structure.name.to_sanitized_pascal_case(), span);
        let str_name_def = Ident::new(&format!("{}Default", str_name), span);

        let mut str_mems = TokenStream::new();
        let mut templ = TokenStream::new();
        let mut default_templ = TokenStream::new();

        let mut where_clause = TokenStream::new();
        let mut inst_default = TokenStream::new();

        for mem in &structure.members {
            match mem {
                StructMember::PrimitiveMember(mem) => {
                    let mem_name = Ident::new(&mem.name.to_sanitized_snake_case(), span);
                    let sty = (mem.bytes * 8).to_ty()?;

                    str_mems.extend(quote! { pub #mem_name : #sty, });
                    inst_default.extend(quote! {
                        #mem_name : 0,
                    });
                }
                StructMember::AlternativesMember(alt) => {
                    let alts = alternatives.get(&alt.alternatives)?;

                    let alt_default = Ident::new(&alts.default.to_sanitized_pascal_case(), span);
                    let alt_name_templ = Ident::new(&alt.name.to_sanitized_pascal_case(), span);
                    let alt_name = Ident::new(&alt.name.to_sanitized_snake_case(), span);
                    let alt_trait = Ident::new(&alt.alternatives.to_sanitized_pascal_case(), span);

                    str_mems.extend(quote! { pub #alt_name : #alt_name_templ, });
                    templ.extend(quote! { #alt_name_templ, });
                    where_clause.extend(quote! { #alt_name_templ : #alt_trait, });
                    inst_default.extend(quote! {
                        #alt_name : #alt_name_templ::default(),
                    });
                    default_templ.extend(quote! { #alt_default, });
                }
            }
        }

        mod_items.extend(quote! {
            #[repr(packed)]
            pub struct #str_name<#templ> where #where_clause {
                #str_mems
            }

            impl<#templ> #str_name<#templ> where #where_clause {
                #[inline(always)]
                pub fn new() -> Self {
                    Self {
                        #inst_default
                    }
                }
            }

        });

        if !default_templ.is_empty() {
            mod_items.extend(quote! {
                pub type #str_name_def = #str_name<#default_templ>;
            });
        }
    }

    mod_items.extend(quote! {#trait_extends});

    Ok(mod_items)
}
