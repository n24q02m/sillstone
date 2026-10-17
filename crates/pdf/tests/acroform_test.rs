//! Test for PDF AcroForm parsing.

use sillstone_pdf::{Document, FieldType};

#[test]
fn test_acroform_parsing() {
    let pdf = b"%PDF-1.4
1 0 obj
<< /Type /Catalog /Pages 2 0 R /AcroForm 4 0 R >>
endobj
2 0 obj
<< /Type /Pages /Kids [3 0 R] /Count 1 >>
endobj
3 0 obj
<< /Type /Page /Parent 2 0 R /MediaBox [0 0 612 792] >>
endobj
4 0 obj
<< /Fields [5 0 R 6 0 R] >>
endobj
5 0 obj
<< /FT /Tx /T (username) /V (alice) >>
endobj
6 0 obj
<< /FT /Btn /T (subscribe) /V /Yes >>
endobj
xref
0 7
0000000000 65535 f 
0000000009 00000 n 
0000000075 00000 n 
0000000132 00000 n 
0000000203 00000 n 
0000000247 00000 n 
0000000300 00000 n 
trailer
<< /Size 7 /Root 1 0 R >>
startxref
353
%%EOF
";

    let doc = Document::open(pdf).expect("open document");
    let acroform = doc.acroform().expect("read acroform").expect("acroform present");
    assert_eq!(acroform.fields.len(), 2);

    assert_eq!(acroform.fields[0].name, "username");
    assert_eq!(acroform.fields[0].field_type, FieldType::Text);
    assert_eq!(acroform.fields[0].value.as_deref(), Some("alice"));

    assert_eq!(acroform.fields[1].name, "subscribe");
    assert_eq!(acroform.fields[1].field_type, FieldType::Button);
    assert_eq!(acroform.fields[1].value.as_deref(), Some("Yes"));
}
