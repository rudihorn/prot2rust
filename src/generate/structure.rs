use anyhow::Result;

use std::collections::HashMap;

use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::util::{ToSanitizedPascalCase, ToSanitizedSnakeCase, U32Ext};

pub trait Type {
    fn name<'a>(&'a self) -> &'a str;
}

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

pub struct BitfieldMember {
    pub name: String,
    pub bitfield: String,
    pub bytes: u32,
}

impl BitfieldMember {
    pub fn new(name: &str, bitfield: &str, bytes: u32) -> Self {
        let name = String::from(name);
        let bitfield = String::from(bitfield);

        Self {
            name,
            bitfield,
            bytes,
        }
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
    BitfieldMember(BitfieldMember),
    PrimitiveMember(PrimitiveMember),
    AlternativesMember(AlternativesMember),
}

impl StructMember {
    pub fn name(&self) -> &str {
        match &self {
            &StructMember::PrimitiveMember(mem) => &mem.name,
            &StructMember::BitfieldMember(mem) => &mem.name,
            &StructMember::AlternativesMember(mem) => &mem.name,
        }
    }
}

#[derive(Clone)]
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

    pub fn insert(mut self, options: &AlternativeOptions) -> Self {
        let key = options.name.clone();
        self.map.insert(key, options.clone());
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

impl Type for Structure {
    fn name<'a>(&'a self) -> &'a str {
        &self.name
    }
}

impl Structure {
    pub fn new(name: &str) -> Structure {
        let name = String::from(name);
        Structure {
            name,
            members: vec![],
        }
    }

    pub fn add_bitfield(mut self, name: &str, bitfield: &str, bytes: u32) -> Self {
        let member = BitfieldMember::new(name, bitfield, bytes);
        self.members.push(StructMember::BitfieldMember(member));
        self
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
        self.add_prim_field(name, 8)
    }

    pub fn add_alt_field(mut self, name: &str, alternatives: &AlternativeOptions) -> Self {
        let member = AlternativesMember::new(name, &alternatives.name);
        self.members.push(StructMember::AlternativesMember(member));
        self
    }
}

pub struct SimpleStructure {
    pub name: String,
    pub member: PrimitiveMember,
}

impl Type for SimpleStructure {
    fn name<'a>(&'a self) -> &'a str {
        &self.name
    }
}

impl SimpleStructure {
    pub fn new(name: &str, mem_name: &str, bytes: u32) -> Self {
        let name = String::from(name);
        Self {
            name,
            member: PrimitiveMember::new(mem_name, bytes),
        }
    }
}

impl AlternativeOptions {
    pub fn new<T>(name: &str, default: &T) -> Self
    where
        T: Type,
    {
        let name = String::from(name);
        let default_name = String::from(default.name());
        Self {
            name,
            default: default_name,
            alternatives: vec![],
        }
        .insert_type(default)
    }

    pub fn insert_type<T>(mut self, structure: &T) -> Self
    where
        T: Type,
    {
        let name = String::from(structure.name());
        self.alternatives.push(name);
        self
    }
}

pub fn render_alternatives(alternatives: &Alternatives) -> Result<TokenStream> {
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
            pub trait #alt_pc : Copy {
                fn default() -> Self;
            }

            enum #alt_pc_a {
                #alt_enum_entries
            }
        });
    }

    mod_items.extend(quote! {#trait_extends});

    Ok(mod_items)
}

pub fn render_simple(structure: &SimpleStructure) -> Result<TokenStream> {
    let mut mod_items = TokenStream::new();

    let span = Span::call_site();
    let str_name = Ident::new(&structure.name.to_sanitized_pascal_case(), span);
    let mem_name = Ident::new(&structure.member.name.to_sanitized_snake_case(), span);
    let sty = (structure.member.bytes * 8).to_ty()?;

    mod_items.extend(quote! {
        #[derive(Clone, Copy, Debug, PartialEq)]
        pub struct #str_name {
            #mem_name : #sty
        }

        impl #str_name {
            pub fn new() -> Self {
                Self { #mem_name : 0 }
            }
            pub fn get(&self) -> #sty {
                self.#mem_name
            }

            pub fn set(&mut self, v : #sty) -> &mut Self {
                self.#mem_name = v;
                self
            }
        }
    });

    Ok(mod_items)
}

pub fn render_with_alts(structure: &Structure, alternatives: &Alternatives) -> Result<TokenStream> {
    let span = Span::call_site();

    let mut mod_items = TokenStream::new();

    let str_name = Ident::new(&structure.name.to_sanitized_pascal_case(), span);
    let str_name_def = Ident::new(&format!("{}Default", str_name), span);

    let mut str_mems = TokenStream::new();
    let mut templ = TokenStream::new();
    let mut default_templ = TokenStream::new();

    let mut where_clause = TokenStream::new();
    let mut inst_default = TokenStream::new();

    let mut str_items = TokenStream::new();
    let mut str_fns = TokenStream::new();

    for mem in &structure.members {
        match mem {
            StructMember::AlternativesMember(alt) => {
                let alts = alternatives.get(&alt.alternatives)?;

                let alt_default = Ident::new(&alts.default.to_sanitized_pascal_case(), span);
                let alt_name_templ =
                    Ident::new(&format!("{}T", alt.name.to_sanitized_pascal_case()), span);
                let alt_trait = Ident::new(&alt.alternatives.to_sanitized_pascal_case(), span);

                templ.extend(quote! { #alt_name_templ, });
                where_clause.extend(quote! { #alt_name_templ : #alt_trait, });
                default_templ.extend(quote! { #alt_default, });
            }
            _ => {}
        }
    }

    for mem in &structure.members {
        let mem_name_str = mem.name();
        let mem_name = Ident::new(&mem_name_str.to_sanitized_snake_case(), span);
        let ty_name = Ident::new(&mem_name_str.to_sanitized_pascal_case(), span);

        let mut mem_str_impl = TokenStream::new();

        let mut default_value = TokenStream::new();
        let mut mem_ty = TokenStream::new();

        match mem {
            StructMember::BitfieldMember(mem) => {
                let pkg_name = Ident::new(&mem.bitfield.to_sanitized_snake_case(), span);
                let sty = (mem.bytes * 8).to_ty()?;

                default_value.extend(quote! { 0 });
                mem_ty.extend(quote! {#sty});

                str_fns.extend(quote! {
                    pub fn #mem_name(&mut self) -> #ty_name<#templ> {
                        #ty_name::new(self)
                    }
                });

                mem_str_impl.extend(quote! {
                        #[inline(always)]
                        pub fn read(&self) -> crate::#pkg_name::R {
                            crate::#pkg_name::R::new(self.data.#mem_name)
                        }

                        #[inline(always)]
                        pub fn modify<F>(&'a mut self, f : F) -> &'a mut #str_name<#templ> where for <'w> F : FnOnce(&'w mut crate::#pkg_name::W) -> &'w mut crate::#pkg_name::W {
                            let bits = self.data.#mem_name;
                            self.data.#mem_name = **f(&mut crate::#pkg_name::W::new(bits));
                            self.data
                        }
                    });
            }
            StructMember::PrimitiveMember(mem) => {
                let sty = (mem.bytes * 8).to_ty()?;

                default_value.extend(quote! { 0 });
                mem_ty.extend(quote! {#sty});

                str_fns.extend(quote! {
                    pub fn #mem_name(&mut self) -> #ty_name<#templ> {
                            #ty_name::new(self)
                        }
                });

                mem_str_impl.extend(quote! {
                    #[inline(always)]
                    pub fn read(&self) -> #sty {
                        self.data.#mem_name
                    }

                    #[inline(always)]
                    pub fn set(&'a mut self, v : #sty) -> &'a mut #str_name<#templ> {
                        self.data.#mem_name = v;
                        self.data
                    }
                });
            }
            StructMember::AlternativesMember(alt) => {
                let alt_name_templ =
                    Ident::new(&format!("{}T", alt.name.to_sanitized_pascal_case()), span);

                default_value.extend(quote! { #alt_name_templ::default() });
                mem_ty.extend(quote! {#alt_name_templ});

                str_fns.extend(quote! {
                    pub fn #mem_name(&mut self) -> #ty_name<#templ> {
                        #ty_name::new(self)
                    }
                });

                mem_str_impl.extend(quote! {
                    #[inline(always)]
                    pub fn read(&self) -> #alt_name_templ {
                        self.data.#mem_name
                    }

                    #[inline(always)]
                    pub fn modify<F>(&'a mut self, f : F) -> &'a mut #str_name<#templ> where for <'w> F : FnOnce(&'w mut #alt_name_templ) -> &'w mut #alt_name_templ {
                        let mut cp = self.data.#mem_name;
                        self.data.#mem_name = *f(&mut cp);
                        self.data
                    }
                });
            }
        }

        str_mems.extend(quote! { #mem_name : #mem_ty, });
        inst_default.extend(quote! {
            #mem_name : #default_value,
        });

        str_items.extend(quote! {
            pub struct #ty_name<'a, #templ> where #where_clause { data : &'a mut #str_name<#templ> }

            impl<'a, #templ> #ty_name<'a, #templ> where #where_clause {
                #[inline(always)]
                pub(crate) fn new(data : &'a mut #str_name<#templ>) -> Self {
                    Self { data }
                }

                #mem_str_impl
            }
        });
    }

    mod_items.extend(quote! {
        #[repr(packed)]
        #[derive(Clone, Copy)]
        pub struct #str_name<#templ> where #where_clause {
            #str_mems
        }

        #str_items

        impl<#templ> #str_name<#templ> where #where_clause {
            #[inline(always)]
            pub fn new() -> Self {
                Self {
                    #inst_default
                }
            }

            #str_fns
        }
    });

    if !default_templ.is_empty() {
        mod_items.extend(quote! {
            pub type #str_name_def = #str_name<#default_templ>;
        });
    }

    Ok(mod_items)
}

pub fn render(structure: &Structure) -> Result<TokenStream> {
    render_with_alts(structure, &Alternatives::new())
}
