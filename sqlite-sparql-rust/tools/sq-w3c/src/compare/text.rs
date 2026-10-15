//! `mf:CSVResultFormatTest`: CSV / TSV result documents compared as text.
//!
//! The documents are split into records and fields (CSV quoting respected; fields keep their
//! exact text), line endings and a trailing newline are ignored, and blank nodes are renamed
//! `_:b0`, `_:b1`, … in order of first appearance. The column order is aligned on the
//! header first: `SELECT *` has no defined column order (spargebra projects it sorted), and
//! these tests are about the serialization, not the projection order.
use std::collections::HashMap;

use crate::results::TextFormat;

type Record = Vec<String>;
type Records = Vec<Record>;

/// Splits a CSV document into records of raw fields (quoted fields keep their quotes).
pub(crate) fn csv_records(text: &str) -> Records {
    let mut records = Vec::new();
    let mut record = Vec::new();
    let mut chars = text.chars().peekable();
    let mut field = String::new();
    let mut quoted = false;
    let mut at_field_start = true;
    while let Some(c) = chars.next() {
        if quoted {
            field.push(c);
            if c == '"' {
                if chars.peek() == Some(&'"') {
                    field.push(chars.next().expect("peeked"));
                } else {
                    quoted = false;
                }
            }
            continue;
        }
        match c {
            '"' if at_field_start => {
                quoted = true;
                field.push(c);
            }
            ',' => {
                record.push(std::mem::take(&mut field));
                at_field_start = true;
                continue;
            }
            '\r' if chars.peek() == Some(&'\n') => {}
            '\n' => {
                record.push(std::mem::take(&mut field));
                records.push(std::mem::take(&mut record));
                at_field_start = true;
                continue;
            }
            c => field.push(c),
        }
        at_field_start = false;
    }
    if !field.is_empty() || !record.is_empty() {
        record.push(field);
        records.push(record);
    }
    records
}

/// The value of a raw CSV field: quotes removed and `""` unescaped.
pub(crate) fn csv_value(field: &str) -> String {
    match field.strip_prefix('"').and_then(|f| f.strip_suffix('"')) {
        Some(inner) => inner.replace("\"\"", "\""),
        None => field.to_owned(),
    }
}

/// Splits a TSV document into records of raw fields (TSV escapes tabs and newlines inside
/// terms, so raw tabs and newlines are always separators).
fn tsv_records(text: &str) -> Records {
    let text = text.replace("\r\n", "\n");
    let text = text.strip_suffix('\n').unwrap_or(&text);
    if text.is_empty() {
        return Vec::new();
    }
    text.split('\n').map(|line| line.split('\t').map(str::to_owned).collect()).collect()
}

fn records(text: &str, format: TextFormat) -> Records {
    match format {
        TextFormat::Csv => csv_records(text),
        TextFormat::Tsv => tsv_records(text),
    }
}

/// Renames blank nodes in row-major order of first appearance.
fn relabel_blank_nodes(records: &mut Records) {
    let mut labels: HashMap<String, usize> = HashMap::new();
    for field in records.iter_mut().skip(1).flatten() {
        if let Some(label) = field.strip_prefix("_:") {
            let n = labels.len();
            let id = *labels.entry(label.to_owned()).or_insert(n);
            *field = format!("_:b{id}");
        }
    }
}

/// Puts the columns of `actual` in the order of `expected`'s header when both headers name
/// the same variables.
fn align_columns(expected: &Records, actual: &mut Records) {
    let (Some(eh), Some(ah)) = (expected.first(), actual.first()) else { return };
    if eh.len() != ah.len() || eh == ah {
        return;
    }
    let Some(order) = eh.iter().map(|name| ah.iter().position(|x| x == name)).collect::<Option<Vec<usize>>>() else {
        return;
    };
    for record in actual.iter_mut().filter(|r| r.len() == order.len()) {
        *record = order.iter().map(|&i| std::mem::take(&mut record[i])).collect();
    }
}

/// Compares two CSV or TSV documents; the error names the first differing record.
pub fn compare_result_text(expected: &str, actual: &str, format: TextFormat) -> Result<(), String> {
    let mut e = records(expected, format);
    let mut a = records(actual, format);
    align_columns(&e, &mut a);
    relabel_blank_nodes(&mut e);
    relabel_blank_nodes(&mut a);
    if e == a {
        return Ok(());
    }
    let separator = match format {
        TextFormat::Csv => ",",
        TextFormat::Tsv => "\t",
    };
    for i in 0..e.len().max(a.len()) {
        let (x, y) = (e.get(i), a.get(i));
        if x != y {
            let show = |r: Option<&Record>| r.map_or_else(|| "<end>".to_owned(), |r| r.join(separator));
            return Err(format!(
                "record {}: expected {:?}, got {:?} ({} vs {} records)",
                i + 1,
                show(x),
                show(y),
                e.len(),
                a.len()
            ));
        }
    }
    Err("documents differ".to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    const CSV: TextFormat = TextFormat::Csv;
    const TSV: TextFormat = TextFormat::Tsv;

    #[test]
    fn csv_records_respect_quoting() {
        let r = csv_records("a,b\r\n\"x,\"\"y\"\"\nz\",_:q\r\n,\r\n");
        assert_eq!(r, [vec!["a", "b"], vec!["\"x,\"\"y\"\"\nz\"", "_:q"], vec!["", ""]]);
        assert_eq!(csv_records("true"), [vec!["true"]]);
        assert!(csv_records("").is_empty());
        assert_eq!(csv_value("\"x,\"\"y\"\"\""), "x,\"y\"");
        assert_eq!(csv_value("plain"), "plain");
    }

    #[test]
    fn relabels_blank_nodes_and_line_endings() {
        let expected = "s,o\nhttp://ex/a,_:a\n_:a,_:zz\n";
        let actual = "s,o\r\nhttp://ex/a,_:b17\r\n_:b17,_:q\r\n";
        compare_result_text(expected, actual, CSV).unwrap();
        // Co-reference matters: one label vs two.
        assert!(compare_result_text("x\n_:a\n_:a\n", "x\n_:a\n_:b\n", CSV).is_err());
    }

    #[test]
    fn aligns_permuted_columns() {
        let expected = "s,p,o\nhttp://ex/s,http://ex/p,_:x\nhttp://ex/t,http://ex/p,\"a,b\"\n";
        let actual = "o,p,s\r\n_:y,http://ex/p,http://ex/s\r\n\"a,b\",http://ex/p,http://ex/t\r\n";
        compare_result_text(expected, actual, CSV).unwrap();
        let expected = "?s\t?o\n<http://ex/s>\t\"x\"\n";
        let actual = "?o\t?s\n\"x\"\t<http://ex/s>\n";
        compare_result_text(expected, actual, TSV).unwrap();
        // Different variables are not aligned.
        assert!(compare_result_text("a,b\n1,2\n", "a,c\n1,2\n", CSV).is_err());
    }

    #[test]
    fn tsv_quoted_fields_and_differences() {
        compare_result_text("?s\t?o\n_:x\t\"_:not a bnode\"\n", "?s\t?o\n_:y\t\"_:not a bnode\"\n", TSV).unwrap();
        let err = compare_result_text("a\n1\n2\n", "a\n1\n3\n", CSV).unwrap_err();
        assert!(err.starts_with("record 3"), "{err}");
        assert!(compare_result_text("?x\n1.0e6\n", "?x\n1.0E6\n", TSV).is_err());
    }
}
