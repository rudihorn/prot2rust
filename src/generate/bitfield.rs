use anyhow::Result;
use proc_macro2::{Ident, Span, TokenStream};
use quote::quote;

use crate::util::{self, ToSanitizedPascalCase, ToSanitizedSnakeCase, U32Ext};

pub struct EnumeratedValue(pub String, pub u64);

pub struct BitFieldMember {
    pub name: String,
    pub desc: String,
    pub bitsize: u32,
    pub enumerated_values: Vec<EnumeratedValue>,
}

impl BitFieldMember {
    pub fn new(name : &str, desc : &str, bitsize : u32) -> BitFieldMember {
        let name = String::from(name);
        let desc = String::from(desc);
        BitFieldMember { name, desc, bitsize, enumerated_values : vec![]}
    }

    pub fn add_enum_value(mut self, name : &str, bits : u64) -> Self {
        let name = String::from(name);
        self.enumerated_values.push(EnumeratedValue(name, bits));
        self
    }
}

pub enum MaybeField {
    Field(BitFieldMember),
    Reserved { bitsize: u32 },
}

pub struct BitField {
    pub name: String,
    pub desc: String,
    pub fields: Vec<MaybeField>,
}

impl BitField {
    pub fn new(name: &str, desc: &str) -> Self {
        let name = String::from(name);
        let desc = String::from(desc);
        Self {
            name,
            desc,
            fields: vec![],
        }
    }

    pub fn add_field(mut self, field : MaybeField) -> Self {
        self.fields.push(field);
        self
    }

    pub fn add_bit_field<F>(self, name : &str, desc : &str, bitsize : u32, mut f : F) -> Self where F : FnMut(BitFieldMember) -> BitFieldMember {
        let field = BitFieldMember::new(name, desc, bitsize);
        let field = f(field);
        self.add_field(MaybeField::Field(field))
    }

    pub fn add_reserved(self, bitsize : u32) -> Self {
        self.add_field(MaybeField::Reserved { bitsize })
    }

}

impl MaybeField {
    pub fn bitsize(&self) -> u32 {
        match &self {
            &MaybeField::Field(field) => field.bitsize,
            &MaybeField::Reserved { bitsize } => *bitsize,
        }
    }
}

pub fn add_field(
    field: &BitFieldMember,
    structsize: u32,
    offset: u32,
    reader_impl: &mut TokenStream,
    writer_impl: &mut TokenStream,
) -> Result<TokenStream> {
    let span = Span::call_site();
    let mut mod_items = TokenStream::new();

    let field_name = field.name.as_str();
    let field_name_sc = Ident::new(&field_name.to_sanitized_snake_case(), span);
    let field_name_pc = Ident::new(&field_name.to_sanitized_pascal_case(), span);
    let field_name_pc_r = Ident::new(&format!("{}R", field_name_pc), span);
    let field_name_pc_w = Ident::new(&format!("{}W", field_name_pc), span);
    let field_name_pc_a = Ident::new(&format!("{}A", field_name_pc), span);
    let field_doc = field.desc.as_str();
    let fty = (field.bitsize as u32).to_ty()?;

    let field_offset = &util::unsuffixed((structsize - offset - field.bitsize) as u64);
    let field_mask = &util::hex(1 << field.bitsize - 1);

    let mut evs = TokenStream::new();
    let mut ev_checkers = TokenStream::new();
    let mut ev_setters = TokenStream::new();
    let mut ev_variants = TokenStream::new();

    for EnumeratedValue(key, val) in &field.enumerated_values {
        let key_pc = Ident::new(&key.to_sanitized_pascal_case(), span);
        let key_sc = Ident::new(&key.to_sanitized_snake_case(), span);
        let is_key_sc = Ident::new(&format!("is_{}", key_sc), span);
        let val_us = util::unsuffixed(val.clone());

        let is_doc = format!("Checks if the value of the `{}` field is `{}`", field_name_pc, key_pc);
        let set_doc = format!("Set the value of the `{}` field to `{}`", field_name_pc, key_pc);

        ev_checkers.extend(quote! {
            #[doc = #is_doc]
            #[inline(always)]
            pub fn #is_key_sc(&self) -> bool {
                **self == #field_name_pc_a::#key_pc
            }
        });

        ev_variants.extend(quote! {
            #val_us => #field_name_pc_a::#key_pc,
        });

        evs.extend(quote! {
            #key_pc = #val_us,
        });

        ev_setters.extend(quote! {
            #[doc = #set_doc]
            #[inline(always)]
            pub fn #key_sc(self) -> &'a mut W {
                self.variant(#field_name_pc_a::#key_pc)
            }
        });
    }

    let field_doc_reader = format!("Field `{}` reader - {}", field_name_pc, field.desc);
    mod_items.extend(quote! {
        #[doc = #field_doc]
        pub enum #field_name_pc_a {
            #evs
        }

        #[doc = #field_doc_reader]
        pub struct #field_name_pc_r(crate::FieldReader<#fty,R>);

        impl #field_name_pc_r {
            #[inline(always)]
            pub(crate) fn new(bits : #fty) -> Self {
                #field_name_pc_r(crate::FieldReader::new(bits))
            }

            #[inline(always)]
            pub fn variant(&self) -> #field_name_pc_a {
                match self.bits {
                    #ev_variants
                }
            }

            #ev_checkers
        }

        pub struct #field_name_pc_w<'a> {
            w : &'a mut W,
        }

        impl<'a> #field_name_pc_w<'a> {
            #[inline(always)]
            pub fn variant(self, variant: #field_name_pc_a) -> &'a mut W {
                self.bits(variant.into())
            }

            #ev_setters

            #[inline(always)]
            pub fn bits(self, value: #fty) -> &'a mut W {
                self.w.bits = (self.w.bits & !(#field_mask << #field_offset)) | ((value as u32 & #field_mask) << #field_offset);
                self.w
            }
        }
    });

    let read_doc = format!("Read the `{}` field.", field_name_pc);
    let set_doc = format!("Set the `{}` field.", field_name_pc);

    reader_impl.extend(quote! {
        #[doc = #read_doc]
        #[inline(always)]
        pub fn #field_name_sc(&self) -> #field_name_pc_r {
            #field_name_pc_r::new(((self.bits >> #field_offset) & #field_mask))
        }
    });

    writer_impl.extend(quote! {
        #[doc = #set_doc]
        #[inline(always)]
        pub fn #field_name_sc(&self) -> #field_name_pc_w {
            #field_name_pc_w { w : self }
        }
    });

    Ok(mod_items)
}

pub fn render(structure: &BitField) -> Result<TokenStream> {
    let desc = structure.desc.as_str();

    let structsize = (structure.fields.iter().map(|v| v.bitsize()).sum::<u32>()).to_ty_width()?;
    let sty = structsize.to_ty()?;

    let mut mod_items = TokenStream::new();
    let mut reader_impl = TokenStream::new();
    let mut writer_impl = TokenStream::new();

    mod_items.extend(quote! {
        #[doc = #desc]
        pub struct R {
            bits : #sty,
        }

        pub struct W {
            bits : #sty,
        }
    });

    let mut offset = 0u32;

    for field in &structure.fields {
        match &field {
            &MaybeField::Field(field) => mod_items.extend(add_field(
                &field,
                structsize,
                offset,
                &mut reader_impl,
                &mut writer_impl,
            )?),
            _ => (),
        }
        offset += field.bitsize()
    }

    mod_items.extend(quote! {
        impl R {
            #reader_impl
        }

        impl W {
            #writer_impl
        }
    });

    Ok(mod_items)
}
