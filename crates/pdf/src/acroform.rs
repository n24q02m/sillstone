//! PDF interactive forms (AcroForm) parsing (ISO 32000-1 §12.7).
//!
//! Extracts form fields, their types (`Tx`, `Btn`, `Ch`, `Sig`) and values (`/V`).

use alloc::string::String;
use alloc::vec::Vec;
use crate::document::{Document, Result};
use crate::object::{Obj, dict_get};

/// The field type (`/FT`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FieldType {
    /// Text field (`/Tx`).
    Text,
    /// Button field (`/Btn`): checkbox, radio button, pushbutton.
    Button,
    /// Choice field (`/Ch`): list box or combo box.
    Choice,
    /// Signature field (`/Sig`).
    Signature,
    /// Another or unrecognized field type.
    Other(String),
}

/// A parsed AcroForm field.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FormField {
    /// Partial or fully-qualified field name (`/T`).
    pub name: String,
    /// Field type (`/FT`).
    pub field_type: FieldType,
    /// Field value (`/V`), if set.
    pub value: Option<String>,
}

/// The document's interactive form structure.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AcroForm {
    /// All terminal form fields in the document.
    pub fields: Vec<FormField>,
}

impl<'a> Document<'a> {
    /// Parses the `/AcroForm` dictionary from the document catalog, if present.
    pub fn acroform(&self) -> Result<Option<AcroForm>> {
        let root = match dict_get(self.trailer(), b"Root").and_then(Obj::as_ref) {
            Some(r) => r,
            None => return Ok(None),
        };
        let catalog = self.object(root)?;
        let catalog_dict = match catalog.dict() {
            Some(d) => d,
            None => return Ok(None),
        };

        let acro_obj = match dict_get(catalog_dict, b"AcroForm") {
            Some(Obj::Ref(r)) => self.object(*r)?,
            Some(o) => o.clone(),
            None => return Ok(None),
        };

        let acro_dict = match acro_obj.dict() {
            Some(d) => d,
            None => return Ok(None),
        };

        let fields_arr = match dict_get(acro_dict, b"Fields") {
            Some(Obj::Ref(r)) => self.object(*r)?,
            Some(o) => o.clone(),
            None => return Ok(Some(AcroForm { fields: Vec::new() })),
        };

        let mut fields = Vec::new();
        if let Some(arr) = fields_arr.as_array() {
            for item in arr {
                self.collect_fields(item, &mut fields)?;
            }
        }

        Ok(Some(AcroForm { fields }))
    }

    fn collect_fields(&self, field_obj: &Obj, out: &mut Vec<FormField>) -> Result<()> {
        let resolved = match field_obj {
            Obj::Ref(r) => self.object(*r)?,
            o => o.clone(),
        };

        let dict = match resolved.dict() {
            Some(d) => d,
            None => return Ok(()),
        };

        // Check if this field has Kids (hierarchical field)
        if let Some(kids) = dict_get(dict, b"Kids").and_then(Obj::as_array) {
            for kid in kids {
                self.collect_fields(kid, out)?;
            }
            return Ok(());
        }

        let name = dict_get(dict, b"T")
            .and_then(|o| match o {
                Obj::Str(s) => core::str::from_utf8(s).ok().map(String::from),
                Obj::Name(n) => core::str::from_utf8(n).ok().map(String::from),
                _ => None,
            })
            .unwrap_or_default();

        let field_type = match dict_get(dict, b"FT") {
            Some(Obj::Name(n)) => match n.as_slice() {
                b"Tx" => FieldType::Text,
                b"Btn" => FieldType::Button,
                b"Ch" => FieldType::Choice,
                b"Sig" => FieldType::Signature,
                other => FieldType::Other(String::from(core::str::from_utf8(other).unwrap_or("?"))),
            },
            _ => FieldType::Other(String::new()),
        };

        let value = dict_get(dict, b"V").and_then(|o| match o {
            Obj::Str(s) => core::str::from_utf8(s).ok().map(String::from),
            Obj::Name(n) => core::str::from_utf8(n).ok().map(String::from),
            Obj::Num(n) => Some(alloc::format!("{}", n)),
            Obj::Bool(b) => Some(alloc::format!("{}", b)),
            _ => None,
        });

        out.push(FormField {
            name,
            field_type,
            value,
        });

        Ok(())
    }
}
