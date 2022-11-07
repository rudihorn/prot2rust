use std::fs::create_dir_all;
use std::fs::File;
use std::io::Write;
use std::path::Path;

use anyhow::Result;
use bitfield::BitField;
use proc_macro2::TokenStream;
use quote::quote;
use structure::{Alternatives, SimpleStructure, Structure};

use crate::generate::bitfield;
use crate::generate::structure;

pub struct GenFile {
    items: TokenStream,
    any: bool,
}

impl GenFile {
    pub fn new() -> Self {
        GenFile {
            items: TokenStream::new(),
            any: false,
        }
    }

    pub fn add_struct_simple(&mut self, s: &SimpleStructure) -> Result<()> {
        self.items.extend(structure::render_simple(s)?);
        Ok(())
    }

    pub fn add_struct_with_alts(&mut self, s: &Structure, alts: &Alternatives) -> Result<()> {
        self.items.extend(structure::render_with_alts(s, alts)?);
        Ok(())
    }

    pub fn add_alternatives(&mut self, alts: &Alternatives) -> Result<()> {
        self.items.extend(structure::render_alternatives(alts)?);
        Ok(())
    }

    pub fn add_struct_imports(&mut self) -> Result<()> {
        self.items.extend(structure::render_imports());
        Ok(())
    }

    pub fn add_struct(&mut self, s: &Structure) -> Result<()> {
        self.items.extend(structure::render(s)?);
        Ok(())
    }

    pub fn add_bitfield(&mut self, bitfield: &BitField) -> Result<()> {
        self.items.extend(bitfield::render(&bitfield)?);
        Ok(())
    }

    pub fn write_file(&self, path: &str) -> Result<()> {
        let path = Path::new(path);
        if let Some(dir) = path.parent() {
            create_dir_all(dir)?;
        }

        let mut file = File::create(path).expect("Could not create output file.");

        let mut dat = TokenStream::new();

        if self.any {
            dat.extend(quote! {
                use core::prelude::rust_2021::derive;
            });
        }

        let items = &self.items;
        dat.extend(quote! {
            #items
        });

        let data = dat.to_string().replace("] ", "]\n");
        file.write_all(data.as_ref())
            .expect("Could not write file.");

        Ok(())
    }
}
