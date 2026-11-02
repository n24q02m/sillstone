//! `Document::acroform` across field types, value kinds, `/Kids` hierarchies and the
//! catalog shapes that yield `None` or an empty form.
//!
//! Each PDF is assembled by `build_pdf` with a correct classic xref (no recovery scan).
//! The expected field lists were cross-checked against pypdf 6.x `PdfReader.get_fields()`
//! on the same bytes: names, `/FT` and `/V` agree (the API here drops the leading `/` of
//! name-valued entries, which pypdf keeps).

use sillstone_pdf::{AcroForm, Document, FieldType, FormField};

/// Builds a one-page PDF. `catalog_extra` is spliced into the catalog dictionary; `objs`
/// maps object numbers (>= 4) to their bodies, with gaps filled by `null`.
fn build_pdf(catalog_extra: &str, objs: &[(u32, &str)]) -> Vec<u8> {
    let mut bodies: Vec<(u32, String)> = vec![
        (
            1,
            format!("<< /Type /Catalog /Pages 2 0 R {catalog_extra} >>"),
        ),
        (2, "<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_string()),
        (
            3,
            "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 200 200] >>".to_string(),
        ),
    ];
    let max = objs.iter().map(|(n, _)| *n).max().unwrap_or(3).max(3);
    for n in 4..=max {
        let body = objs
            .iter()
            .find(|(k, _)| *k == n)
            .map_or("null", |(_, b)| *b);
        bodies.push((n, body.to_string()));
    }

    let mut out = b"%PDF-1.4\n".to_vec();
    let mut offsets = Vec::new();
    for (n, body) in &bodies {
        offsets.push(out.len());
        out.extend_from_slice(format!("{n} 0 obj\n{body}\nendobj\n").as_bytes());
    }
    let size = max + 1;
    let xref = out.len();
    out.extend_from_slice(format!("xref\n0 {size}\n0000000000 65535 f \n").as_bytes());
    for off in &offsets {
        out.extend_from_slice(format!("{off:010} 00000 n \n").as_bytes());
    }
    out.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    out
}

fn field(name: &str, field_type: FieldType, value: Option<&str>) -> FormField {
    FormField {
        name: name.to_string(),
        field_type,
        value: value.map(str::to_string),
    }
}

fn acroform_of(pdf: &[u8]) -> Option<AcroForm> {
    Document::open(pdf)
        .expect("open document")
        .acroform()
        .expect("read acroform")
}

#[test]
fn field_types_and_value_kinds_via_indirect_fields_array() {
    // `/AcroForm` and `/Fields` are both indirect references here.
    let pdf = build_pdf(
        "/AcroForm 4 0 R",
        &[
            (4, "<< /Fields 5 0 R >>"),
            (5, "[6 0 R 7 0 R 8 0 R 9 0 R 10 0 R 11 0 R 12 0 R]"),
            (6, "<< /FT /Ch /T (color) /V (red) >>"),
            (7, "<< /FT /Sig /T (sign) >>"),
            (8, "<< /FT /Foo /T (weird) /V 42 >>"),
            (9, "<< /FT /Tx /T /NameT /V true >>"),
            (10, "<< /FT /Tx /T (ratio) /V 3.5 >>"),
            (11, "<< /FT /Tx /T (arr) /V [(a) (b)] >>"),
            (12, "<< /T (notype) /V (x) >>"),
        ],
    );
    let form = acroform_of(&pdf).expect("acroform present");
    assert_eq!(
        form.fields,
        vec![
            field("color", FieldType::Choice, Some("red")),
            field("sign", FieldType::Signature, None),
            field("weird", FieldType::Other("Foo".into()), Some("42")),
            field("NameT", FieldType::Text, Some("true")),
            field("ratio", FieldType::Text, Some("3.5")),
            // An array value is not a scalar: no value is reported.
            field("arr", FieldType::Text, None),
            // No `/FT`: reported as an empty `Other`.
            field("notype", FieldType::Other(String::new()), Some("x")),
        ]
    );
}

#[test]
fn kids_hierarchy_yields_only_terminal_fields() {
    let pdf = build_pdf(
        "/AcroForm << /Fields [5 0 R] >>",
        &[
            (5, "<< /T (form) /Kids [6 0 R 7 0 R] >>"),
            (6, "<< /FT /Tx /T (first) /V (a) >>"),
            (7, "<< /T (group) /Kids [8 0 R] >>"),
            (8, "<< /FT /Btn /T (deep) /V /On >>"),
        ],
    );
    let form = acroform_of(&pdf).expect("acroform present");
    // The intermediate nodes `form` and `group` are not reported, only the terminals,
    // in document order, with their own partial names.
    assert_eq!(
        form.fields,
        vec![
            field("first", FieldType::Text, Some("a")),
            field("deep", FieldType::Button, Some("On")),
        ]
    );
}

#[test]
fn non_dictionary_entry_in_fields_is_skipped() {
    let pdf = build_pdf(
        "/AcroForm 4 0 R",
        &[
            (4, "<< /Fields [5 0 R 6 0 R 7 0 R] >>"),
            (5, "<< /FT /Tx /T (before) /V (1) >>"),
            (6, "7"),
            (7, "<< /FT /Tx /T (after) /V (2) >>"),
        ],
    );
    let form = acroform_of(&pdf).expect("acroform present");
    assert_eq!(
        form.fields,
        vec![
            field("before", FieldType::Text, Some("1")),
            field("after", FieldType::Text, Some("2")),
        ]
    );
}

#[test]
fn acroform_without_fields_key_is_an_empty_form() {
    let pdf = build_pdf("/AcroForm << /NeedAppearances true >>", &[]);
    let form = acroform_of(&pdf).expect("acroform present");
    assert!(form.fields.is_empty());
}

#[test]
fn acroform_that_is_not_a_dictionary_is_none() {
    let pdf = build_pdf("/AcroForm 42", &[]);
    assert_eq!(acroform_of(&pdf), None);
}

#[test]
fn catalog_without_acroform_is_none() {
    let pdf = build_pdf("", &[]);
    assert_eq!(acroform_of(&pdf), None);
}

#[test]
fn direct_field_dictionary_and_untitled_field_are_tolerated() {
    // The standard wants indirect references here (pypdf rejects this file); the reader is
    // lenient and accepts a direct dictionary. A `/T` that is neither string nor name gives
    // an empty name rather than dropping the field.
    let pdf = build_pdf(
        "/AcroForm << /Fields [<< /FT /Tx /T (inline) /V (z) >> 4 0 R] >>",
        &[(4, "<< /FT /Tx /T 5 /V (untitled) >>")],
    );
    let form = acroform_of(&pdf).expect("acroform present");
    assert_eq!(
        form.fields,
        vec![
            field("inline", FieldType::Text, Some("z")),
            field("", FieldType::Text, Some("untitled")),
        ]
    );
}

#[test]
fn fields_that_is_not_an_array_gives_an_empty_form() {
    let pdf = build_pdf("/AcroForm 4 0 R", &[(4, "<< /Fields 5 0 R >>"), (5, "42")]);
    let form = acroform_of(&pdf).expect("acroform present");
    assert!(form.fields.is_empty());
}
