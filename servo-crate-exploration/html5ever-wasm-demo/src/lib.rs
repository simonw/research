//! Tiny WebAssembly SPA that runs Servo's `html5ever` HTML parser
//! entirely in the browser and returns a normalised pretty-printed
//! view of the document tree.
//!
//! The point of this crate is to demonstrate a concrete, useful
//! thing you can build out of Servo sub-crates that already compile
//! to wasm32, even though the whole `servo` crate does not.
//!
//! The "whole Servo engine to wasm" story is covered in the top-level
//! README; TL;DR: SpiderMonkey + threading + native GL rule it out.
//! But `html5ever` + `markup5ever_rcdom` build cleanly on
//! `wasm32-unknown-unknown` and give you a real, spec-compliant HTML5
//! tokenizer / tree-constructor running in the user's tab.

use std::cell::RefCell;
use std::rc::Rc;

use html5ever::driver::ParseOpts;
use html5ever::tendril::TendrilSink;
use html5ever::tree_builder::TreeBuilderOpts;
use html5ever::{parse_document, serialize, QualName};
use markup5ever_rcdom::{Handle, NodeData, RcDom, SerializableHandle};
use wasm_bindgen::prelude::*;

/// Parse an HTML document and return a compact indented tree dump
/// that shows how html5ever actually interpreted the markup. This
/// makes the quirks-mode / implicit-tags behaviour visible, which is
/// what makes this interesting compared to a naive text formatter.
#[wasm_bindgen]
pub fn parse_tree(html: &str) -> String {
    let opts = ParseOpts {
        tree_builder: TreeBuilderOpts {
            drop_doctype: false,
            ..Default::default()
        },
        ..Default::default()
    };
    let dom = parse_document(RcDom::default(), opts)
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| RcDom::default());

    let mut out = String::new();
    dump_tree(&dom.document, 0, &mut out);
    // Surface parser errors at the bottom. These are the actual HTML5
    // parser error messages servo would emit.
    if !dom.errors.borrow().is_empty() {
        out.push_str("\n---- parser errors ----\n");
        for err in dom.errors.borrow().iter() {
            out.push_str(&format!("- {}\n", err));
        }
    }
    out
}

/// Parse `html` and re-serialise it to a normalised form (implicit
/// <html>/<head>/<body> inserted, attributes quoted, entities
/// escaped). This is what you'd ship an end-user as the
/// "sanitiser / canonicaliser" output.
#[wasm_bindgen]
pub fn normalise(html: &str) -> String {
    let dom = parse_document(RcDom::default(), ParseOpts::default())
        .from_utf8()
        .read_from(&mut html.as_bytes())
        .unwrap_or_else(|_| RcDom::default());

    let mut buf: Vec<u8> = Vec::new();
    let doc: SerializableHandle = dom.document.into();
    let _ = serialize(&mut buf, &doc, Default::default());
    String::from_utf8_lossy(&buf).into_owned()
}

fn indent(n: usize, out: &mut String) {
    for _ in 0..n {
        out.push_str("  ");
    }
}

fn dump_tree(handle: &Handle, depth: usize, out: &mut String) {
    let node = handle;
    match &node.data {
        NodeData::Document => out.push_str("#document\n"),
        NodeData::Doctype { name, .. } => {
            indent(depth, out);
            out.push_str(&format!("<!DOCTYPE {}>\n", name));
        }
        NodeData::Text { contents } => {
            let text = contents.borrow();
            let text = text.trim();
            if !text.is_empty() {
                indent(depth, out);
                out.push_str(&format!("\"{}\"\n", shorten(text)));
            }
        }
        NodeData::Comment { contents } => {
            indent(depth, out);
            out.push_str(&format!("<!-- {} -->\n", shorten(contents)));
        }
        NodeData::Element { name, attrs, .. } => {
            indent(depth, out);
            out.push('<');
            push_qname(name, out);
            for a in attrs.borrow().iter() {
                out.push(' ');
                push_qname(&a.name, out);
                out.push_str(&format!("=\"{}\"", a.value));
            }
            out.push_str(">\n");
        }
        NodeData::ProcessingInstruction { .. } => {}
    }
    for child in node.children.borrow().iter() {
        dump_tree(child, depth + 1, out);
    }
}

fn push_qname(q: &QualName, out: &mut String) {
    // We intentionally drop the namespace prefix for readability;
    // html5ever tags are almost always in the html namespace.
    out.push_str(&q.local);
}

fn shorten(s: &str) -> String {
    let s = s.replace('\n', "\\n");
    if s.len() > 200 {
        format!("{}…", &s[..200])
    } else {
        s
    }
}

// Give the module a tiny marker so the bundler can't dead-code it.
#[wasm_bindgen(start)]
pub fn init() {
    // Force `html5ever`'s Rc<RefCell<...>> types to be referenced so
    // the link step can't eat them in rare tree-shake corner cases.
    let _ = Rc::new(RefCell::new(()));
}
