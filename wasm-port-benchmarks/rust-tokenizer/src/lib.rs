use wasm_bindgen::prelude::*;
use serde::{Serialize, Deserialize};

/// Token types that can be emitted
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Token {
    #[serde(rename = "start")]
    StartTag { name: String, attrs: Vec<(String, String)>, self_closing: bool },
    #[serde(rename = "end")]
    EndTag { name: String },
    #[serde(rename = "text")]
    Text { data: String },
    #[serde(rename = "comment")]
    Comment { data: String },
    #[serde(rename = "doctype")]
    Doctype { name: Option<String>, public_id: Option<String>, system_id: Option<String> },
    #[serde(rename = "eof")]
    EOF,
}

/// Tokenizer states following HTML5 spec
#[derive(Clone, Copy, Debug, PartialEq)]
enum State {
    Data,
    TagOpen,
    EndTagOpen,
    TagName,
    BeforeAttributeName,
    AttributeName,
    AfterAttributeName,
    BeforeAttributeValue,
    AttributeValueDoubleQuoted,
    AttributeValueSingleQuoted,
    AttributeValueUnquoted,
    AfterAttributeValueQuoted,
    SelfClosingStartTag,
    MarkupDeclarationOpen,
    CommentStart,
    CommentStartDash,
    Comment,
    CommentEndDash,
    CommentEnd,
    CommentEndBang,
    BogusComment,
    Doctype,
    BeforeDoctypeName,
    DoctypeName,
    AfterDoctypeName,
    BogusDoctype,
    AfterDoctypePublicKeyword,
    AfterDoctypeSystemKeyword,
    BeforeDoctypePublicIdentifier,
    DoctypePublicIdentifierDoubleQuoted,
    DoctypePublicIdentifierSingleQuoted,
    AfterDoctypePublicIdentifier,
    BetweenDoctypePublicAndSystemIdentifiers,
    BeforeDoctypeSystemIdentifier,
    DoctypeSystemIdentifierDoubleQuoted,
    DoctypeSystemIdentifierSingleQuoted,
    AfterDoctypeSystemIdentifier,
    CdataSection,
    CdataSectionBracket,
    CdataSectionEnd,
    Rcdata,
    RcdataLessThanSign,
    RcdataEndTagOpen,
    RcdataEndTagName,
    Rawtext,
    RawtextLessThanSign,
    RawtextEndTagOpen,
    RawtextEndTagName,
    Plaintext,
}

const RCDATA_ELEMENTS: &[&str] = &["title", "textarea"];
const RAWTEXT_ELEMENTS: &[&str] = &["script", "style", "xmp", "iframe", "noembed", "noframes"];

#[inline]
fn is_whitespace(c: char) -> bool {
    matches!(c, '\t' | '\n' | '\x0C' | ' ' | '\r')
}

#[inline]
fn is_ascii_alpha(c: char) -> bool {
    c.is_ascii_alphabetic()
}

#[inline]
fn ascii_lower(c: char) -> char {
    c.to_ascii_lowercase()
}

/// High-performance HTML5 tokenizer
pub struct Tokenizer {
    input: Vec<char>,
    pos: usize,
    state: State,

    // Current character handling
    current_char: Option<char>,
    reconsume: bool,
    ignore_lf: bool,

    // Token construction buffers
    text_buffer: String,
    current_tag_name: String,
    current_tag_attrs: Vec<(String, String)>,
    current_attr_name: String,
    current_attr_value: String,
    current_tag_self_closing: bool,
    is_end_tag: bool,

    // Comment buffer
    current_comment: String,

    // Doctype fields
    doctype_name: String,
    doctype_public: Option<String>,
    doctype_system: Option<String>,
    doctype_force_quirks: bool,

    // For rawtext/rcdata handling
    last_start_tag_name: Option<String>,
    rawtext_tag_name: Option<String>,
    original_tag_name: String,
    temp_buffer: String,

    // Output tokens
    tokens: Vec<Token>,
}

impl Tokenizer {
    pub fn new(html: &str) -> Self {
        let input: Vec<char> = html.chars().collect();
        Tokenizer {
            input,
            pos: 0,
            state: State::Data,
            current_char: None,
            reconsume: false,
            ignore_lf: false,
            text_buffer: String::with_capacity(1024),
            current_tag_name: String::with_capacity(32),
            current_tag_attrs: Vec::with_capacity(8),
            current_attr_name: String::with_capacity(32),
            current_attr_value: String::with_capacity(128),
            current_tag_self_closing: false,
            is_end_tag: false,
            current_comment: String::with_capacity(256),
            doctype_name: String::with_capacity(16),
            doctype_public: None,
            doctype_system: None,
            doctype_force_quirks: false,
            last_start_tag_name: None,
            rawtext_tag_name: None,
            original_tag_name: String::with_capacity(32),
            temp_buffer: String::with_capacity(32),
            tokens: Vec::with_capacity(256),
        }
    }

    #[inline]
    fn get_char(&mut self) -> Option<char> {
        if self.reconsume {
            self.reconsume = false;
            return self.current_char;
        }

        loop {
            if self.pos >= self.input.len() {
                self.current_char = None;
                return None;
            }

            let c = self.input[self.pos];
            self.pos += 1;

            if c == '\r' {
                self.ignore_lf = true;
                self.current_char = Some('\n');
                return Some('\n');
            } else if c == '\n' && self.ignore_lf {
                self.ignore_lf = false;
                continue;
            } else {
                self.ignore_lf = false;
                self.current_char = Some(c);
                return Some(c);
            }
        }
    }

    #[inline]
    fn reconsume_current(&mut self) {
        self.reconsume = true;
    }

    #[inline]
    fn append_text(&mut self, c: char) {
        self.text_buffer.push(c);
    }

    fn flush_text(&mut self) {
        if !self.text_buffer.is_empty() {
            let data = std::mem::take(&mut self.text_buffer);
            self.tokens.push(Token::Text { data });
        }
    }

    fn emit_current_tag(&mut self) {
        let name = std::mem::take(&mut self.current_tag_name);
        let attrs = std::mem::take(&mut self.current_tag_attrs);

        if self.is_end_tag {
            self.tokens.push(Token::EndTag { name: name.clone() });
        } else {
            // Check if we need to switch to rawtext/rcdata mode
            if RCDATA_ELEMENTS.contains(&name.as_str()) {
                self.state = State::Rcdata;
                self.rawtext_tag_name = Some(name.clone());
            } else if RAWTEXT_ELEMENTS.contains(&name.as_str()) {
                self.state = State::Rawtext;
                self.rawtext_tag_name = Some(name.clone());
            } else if name == "plaintext" {
                self.state = State::Plaintext;
            } else {
                self.state = State::Data;
            }

            self.last_start_tag_name = Some(name.clone());
            self.tokens.push(Token::StartTag {
                name,
                attrs,
                self_closing: self.current_tag_self_closing
            });
        }

        self.current_tag_self_closing = false;
        self.is_end_tag = false;
    }

    fn emit_comment(&mut self) {
        let data = std::mem::take(&mut self.current_comment);
        self.tokens.push(Token::Comment { data });
    }

    fn emit_doctype(&mut self) {
        let name = if self.doctype_name.is_empty() {
            None
        } else {
            Some(std::mem::take(&mut self.doctype_name))
        };
        let public_id = self.doctype_public.take();
        let system_id = self.doctype_system.take();
        self.doctype_force_quirks = false;
        self.tokens.push(Token::Doctype { name, public_id, system_id });
    }

    fn finish_attribute(&mut self) {
        if !self.current_attr_name.is_empty() {
            let name = std::mem::take(&mut self.current_attr_name);
            let value = std::mem::take(&mut self.current_attr_value);

            // Don't add duplicate attributes
            if !self.current_tag_attrs.iter().any(|(n, _)| n == &name) {
                self.current_tag_attrs.push((name, value));
            }
        }
        self.current_attr_name.clear();
        self.current_attr_value.clear();
    }

    fn consume_if(&mut self, literal: &str) -> bool {
        let chars: Vec<char> = literal.chars().collect();
        let end = self.pos + chars.len();
        if end > self.input.len() {
            return false;
        }

        for (i, &c) in chars.iter().enumerate() {
            if self.input[self.pos + i] != c {
                return false;
            }
        }

        self.pos = end;
        true
    }

    fn consume_case_insensitive(&mut self, literal: &str) -> bool {
        let chars: Vec<char> = literal.chars().collect();
        let end = self.pos + chars.len();
        if end > self.input.len() {
            return false;
        }

        for (i, &c) in chars.iter().enumerate() {
            if self.input[self.pos + i].to_ascii_lowercase() != c.to_ascii_lowercase() {
                return false;
            }
        }

        self.pos = end;
        true
    }

    /// Main tokenization loop
    pub fn tokenize(&mut self) -> &[Token] {
        loop {
            if self.step() {
                break;
            }
        }
        &self.tokens
    }

    /// Single step of the tokenizer - returns true when EOF
    fn step(&mut self) -> bool {
        match self.state {
            State::Data => self.state_data(),
            State::TagOpen => self.state_tag_open(),
            State::EndTagOpen => self.state_end_tag_open(),
            State::TagName => self.state_tag_name(),
            State::BeforeAttributeName => self.state_before_attribute_name(),
            State::AttributeName => self.state_attribute_name(),
            State::AfterAttributeName => self.state_after_attribute_name(),
            State::BeforeAttributeValue => self.state_before_attribute_value(),
            State::AttributeValueDoubleQuoted => self.state_attribute_value_double(),
            State::AttributeValueSingleQuoted => self.state_attribute_value_single(),
            State::AttributeValueUnquoted => self.state_attribute_value_unquoted(),
            State::AfterAttributeValueQuoted => self.state_after_attribute_value_quoted(),
            State::SelfClosingStartTag => self.state_self_closing_start_tag(),
            State::MarkupDeclarationOpen => self.state_markup_declaration_open(),
            State::CommentStart => self.state_comment_start(),
            State::CommentStartDash => self.state_comment_start_dash(),
            State::Comment => self.state_comment(),
            State::CommentEndDash => self.state_comment_end_dash(),
            State::CommentEnd => self.state_comment_end(),
            State::CommentEndBang => self.state_comment_end_bang(),
            State::BogusComment => self.state_bogus_comment(),
            State::Doctype => self.state_doctype(),
            State::BeforeDoctypeName => self.state_before_doctype_name(),
            State::DoctypeName => self.state_doctype_name(),
            State::AfterDoctypeName => self.state_after_doctype_name(),
            State::BogusDoctype => self.state_bogus_doctype(),
            State::AfterDoctypePublicKeyword => self.state_after_doctype_public_keyword(),
            State::AfterDoctypeSystemKeyword => self.state_after_doctype_system_keyword(),
            State::BeforeDoctypePublicIdentifier => self.state_before_doctype_public_identifier(),
            State::DoctypePublicIdentifierDoubleQuoted => self.state_doctype_public_identifier_double_quoted(),
            State::DoctypePublicIdentifierSingleQuoted => self.state_doctype_public_identifier_single_quoted(),
            State::AfterDoctypePublicIdentifier => self.state_after_doctype_public_identifier(),
            State::BetweenDoctypePublicAndSystemIdentifiers => self.state_between_doctype_public_and_system_identifiers(),
            State::BeforeDoctypeSystemIdentifier => self.state_before_doctype_system_identifier(),
            State::DoctypeSystemIdentifierDoubleQuoted => self.state_doctype_system_identifier_double_quoted(),
            State::DoctypeSystemIdentifierSingleQuoted => self.state_doctype_system_identifier_single_quoted(),
            State::AfterDoctypeSystemIdentifier => self.state_after_doctype_system_identifier(),
            State::CdataSection => self.state_cdata_section(),
            State::CdataSectionBracket => self.state_cdata_section_bracket(),
            State::CdataSectionEnd => self.state_cdata_section_end(),
            State::Rcdata => self.state_rcdata(),
            State::RcdataLessThanSign => self.state_rcdata_less_than_sign(),
            State::RcdataEndTagOpen => self.state_rcdata_end_tag_open(),
            State::RcdataEndTagName => self.state_rcdata_end_tag_name(),
            State::Rawtext => self.state_rawtext(),
            State::RawtextLessThanSign => self.state_rawtext_less_than_sign(),
            State::RawtextEndTagOpen => self.state_rawtext_end_tag_open(),
            State::RawtextEndTagName => self.state_rawtext_end_tag_name(),
            State::Plaintext => self.state_plaintext(),
        }
    }

    // State handler implementations

    fn state_data(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('<') => {
                self.flush_text();
                self.state = State::TagOpen;
                false
            }
            Some(c) => {
                self.append_text(c);
                false
            }
        }
    }

    fn state_tag_open(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.append_text('<');
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('!') => {
                self.state = State::MarkupDeclarationOpen;
                false
            }
            Some('/') => {
                self.state = State::EndTagOpen;
                false
            }
            Some('?') => {
                self.current_comment.clear();
                self.reconsume_current();
                self.state = State::BogusComment;
                false
            }
            Some(c) if is_ascii_alpha(c) => {
                self.is_end_tag = false;
                self.current_tag_name.clear();
                self.current_tag_name.push(ascii_lower(c));
                self.current_tag_attrs.clear();
                self.current_tag_self_closing = false;
                self.state = State::TagName;
                false
            }
            Some(_) => {
                self.append_text('<');
                self.reconsume_current();
                self.state = State::Data;
                false
            }
        }
    }

    fn state_end_tag_open(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.text_buffer.push_str("</");
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_ascii_alpha(c) => {
                self.is_end_tag = true;
                self.current_tag_name.clear();
                self.current_tag_name.push(ascii_lower(c));
                self.current_tag_attrs.clear();
                self.current_tag_self_closing = false;
                self.state = State::TagName;
                false
            }
            Some('>') => {
                self.state = State::Data;
                false
            }
            Some(_) => {
                self.current_comment.clear();
                self.reconsume_current();
                self.state = State::BogusComment;
                false
            }
        }
    }

    fn state_tag_name(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => {
                self.state = State::BeforeAttributeName;
                false
            }
            Some('/') => {
                self.state = State::SelfClosingStartTag;
                false
            }
            Some('>') => {
                self.emit_current_tag();
                false
            }
            Some('\0') => {
                self.current_tag_name.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_tag_name.push(ascii_lower(c));
                false
            }
        }
    }

    fn state_before_attribute_name(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => false,
            Some('/') => {
                self.state = State::SelfClosingStartTag;
                false
            }
            Some('>') => {
                self.emit_current_tag();
                false
            }
            Some('=') => {
                self.current_attr_name.clear();
                self.current_attr_name.push('=');
                self.current_attr_value.clear();
                self.state = State::AttributeName;
                false
            }
            Some(_) => {
                self.current_attr_name.clear();
                self.current_attr_value.clear();
                self.reconsume_current();
                self.state = State::AttributeName;
                false
            }
        }
    }

    fn state_attribute_name(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => {
                self.finish_attribute();
                self.state = State::AfterAttributeName;
                false
            }
            Some('/') => {
                self.finish_attribute();
                self.state = State::SelfClosingStartTag;
                false
            }
            Some('=') => {
                self.state = State::BeforeAttributeValue;
                false
            }
            Some('>') => {
                self.finish_attribute();
                self.emit_current_tag();
                false
            }
            Some('\0') => {
                self.current_attr_name.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_attr_name.push(ascii_lower(c));
                false
            }
        }
    }

    fn state_after_attribute_name(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => false,
            Some('/') => {
                self.state = State::SelfClosingStartTag;
                false
            }
            Some('=') => {
                self.state = State::BeforeAttributeValue;
                false
            }
            Some('>') => {
                self.emit_current_tag();
                false
            }
            Some(_) => {
                self.current_attr_name.clear();
                self.current_attr_value.clear();
                self.reconsume_current();
                self.state = State::AttributeName;
                false
            }
        }
    }

    fn state_before_attribute_value(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => false,
            Some('"') => {
                self.state = State::AttributeValueDoubleQuoted;
                false
            }
            Some('\'') => {
                self.state = State::AttributeValueSingleQuoted;
                false
            }
            Some('>') => {
                self.finish_attribute();
                self.emit_current_tag();
                false
            }
            Some(_) => {
                self.reconsume_current();
                self.state = State::AttributeValueUnquoted;
                false
            }
        }
    }

    fn state_attribute_value_double(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('"') => {
                self.finish_attribute();
                self.state = State::AfterAttributeValueQuoted;
                false
            }
            Some('\0') => {
                self.current_attr_value.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_attr_value.push(c);
                false
            }
        }
    }

    fn state_attribute_value_single(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('\'') => {
                self.finish_attribute();
                self.state = State::AfterAttributeValueQuoted;
                false
            }
            Some('\0') => {
                self.current_attr_value.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_attr_value.push(c);
                false
            }
        }
    }

    fn state_attribute_value_unquoted(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => {
                self.finish_attribute();
                self.state = State::BeforeAttributeName;
                false
            }
            Some('>') => {
                self.finish_attribute();
                self.emit_current_tag();
                false
            }
            Some('\0') => {
                self.current_attr_value.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_attr_value.push(c);
                false
            }
        }
    }

    fn state_after_attribute_value_quoted(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => {
                self.state = State::BeforeAttributeName;
                false
            }
            Some('/') => {
                self.state = State::SelfClosingStartTag;
                false
            }
            Some('>') => {
                self.emit_current_tag();
                false
            }
            Some(_) => {
                self.reconsume_current();
                self.state = State::BeforeAttributeName;
                false
            }
        }
    }

    fn state_self_closing_start_tag(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('>') => {
                self.current_tag_self_closing = true;
                self.emit_current_tag();
                false
            }
            Some(_) => {
                self.reconsume_current();
                self.state = State::BeforeAttributeName;
                false
            }
        }
    }

    fn state_markup_declaration_open(&mut self) -> bool {
        if self.consume_if("--") {
            self.current_comment.clear();
            self.state = State::CommentStart;
            return false;
        }

        if self.consume_case_insensitive("DOCTYPE") {
            self.doctype_name.clear();
            self.doctype_public = None;
            self.doctype_system = None;
            self.doctype_force_quirks = false;
            self.state = State::Doctype;
            return false;
        }

        if self.consume_if("[CDATA[") {
            // Treat as bogus comment in HTML context
            self.current_comment.clear();
            self.current_comment.push_str("[CDATA[");
            self.state = State::BogusComment;
            return false;
        }

        self.current_comment.clear();
        self.state = State::BogusComment;
        false
    }

    fn state_comment_start(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('-') => {
                self.state = State::CommentStartDash;
                false
            }
            Some('>') => {
                self.emit_comment();
                self.state = State::Data;
                false
            }
            Some('\0') => {
                self.current_comment.push('\u{FFFD}');
                self.state = State::Comment;
                false
            }
            Some(c) => {
                self.current_comment.push(c);
                self.state = State::Comment;
                false
            }
        }
    }

    fn state_comment_start_dash(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('-') => {
                self.state = State::CommentEnd;
                false
            }
            Some('>') => {
                self.emit_comment();
                self.state = State::Data;
                false
            }
            Some('\0') => {
                self.current_comment.push('-');
                self.current_comment.push('\u{FFFD}');
                self.state = State::Comment;
                false
            }
            Some(c) => {
                self.current_comment.push('-');
                self.current_comment.push(c);
                self.state = State::Comment;
                false
            }
        }
    }

    fn state_comment(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('-') => {
                self.state = State::CommentEndDash;
                false
            }
            Some('\0') => {
                self.current_comment.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_comment.push(c);
                false
            }
        }
    }

    fn state_comment_end_dash(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('-') => {
                self.state = State::CommentEnd;
                false
            }
            Some('\0') => {
                self.current_comment.push('-');
                self.current_comment.push('\u{FFFD}');
                self.state = State::Comment;
                false
            }
            Some(c) => {
                self.current_comment.push('-');
                self.current_comment.push(c);
                self.state = State::Comment;
                false
            }
        }
    }

    fn state_comment_end(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('>') => {
                self.emit_comment();
                self.state = State::Data;
                false
            }
            Some('!') => {
                self.state = State::CommentEndBang;
                false
            }
            Some('-') => {
                self.current_comment.push('-');
                false
            }
            Some('\0') => {
                self.current_comment.push_str("--\u{FFFD}");
                self.state = State::Comment;
                false
            }
            Some(c) => {
                self.current_comment.push_str("--");
                self.current_comment.push(c);
                self.state = State::Comment;
                false
            }
        }
    }

    fn state_comment_end_bang(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('-') => {
                self.current_comment.push_str("--!");
                self.state = State::CommentEndDash;
                false
            }
            Some('>') => {
                self.emit_comment();
                self.state = State::Data;
                false
            }
            Some('\0') => {
                self.current_comment.push_str("--!\u{FFFD}");
                self.state = State::Comment;
                false
            }
            Some(c) => {
                self.current_comment.push_str("--!");
                self.current_comment.push(c);
                self.state = State::Comment;
                false
            }
        }
    }

    fn state_bogus_comment(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.emit_comment();
                self.tokens.push(Token::EOF);
                true
            }
            Some('>') => {
                self.emit_comment();
                self.state = State::Data;
                false
            }
            Some('\0') => {
                self.current_comment.push('\u{FFFD}');
                false
            }
            Some(c) => {
                self.current_comment.push(c);
                false
            }
        }
    }

    fn state_doctype(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.doctype_force_quirks = true;
                self.emit_doctype();
                self.tokens.push(Token::EOF);
                true
            }
            Some(c) if is_whitespace(c) => {
                self.state = State::BeforeDoctypeName;
                false
            }
            Some('>') => {
                self.doctype_force_quirks = true;
                self.emit_doctype();
                self.state = State::Data;
                false
            }
            Some(_) => {
                self.reconsume_current();
                self.state = State::BeforeDoctypeName;
                false
            }
        }
    }

    fn state_before_doctype_name(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => continue,
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(c) if c.is_ascii_uppercase() => {
                    self.doctype_name.push(c.to_ascii_lowercase());
                    self.state = State::DoctypeName;
                    return false;
                }
                Some('\0') => {
                    self.doctype_name.push('\u{FFFD}');
                    self.state = State::DoctypeName;
                    return false;
                }
                Some(c) => {
                    self.doctype_name.push(c);
                    self.state = State::DoctypeName;
                    return false;
                }
            }
        }
    }

    fn state_doctype_name(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => {
                    self.state = State::AfterDoctypeName;
                    return false;
                }
                Some('>') => {
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(c) if c.is_ascii_uppercase() => {
                    self.doctype_name.push(c.to_ascii_lowercase());
                }
                Some('\0') => {
                    self.doctype_name.push('\u{FFFD}');
                }
                Some(c) => {
                    self.doctype_name.push(c);
                }
            }
        }
    }

    fn state_after_doctype_name(&mut self) -> bool {
        if self.consume_case_insensitive("PUBLIC") {
            self.state = State::AfterDoctypePublicKeyword;
            return false;
        }
        if self.consume_case_insensitive("SYSTEM") {
            self.state = State::AfterDoctypeSystemKeyword;
            return false;
        }

        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => continue,
                Some('>') => {
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_bogus_doctype(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some('>') => {
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {}
            }
        }
    }

    fn state_after_doctype_public_keyword(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => {
                    self.state = State::BeforeDoctypePublicIdentifier;
                    return false;
                }
                Some('"') => {
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdentifierDoubleQuoted;
                    return false;
                }
                Some('\'') => {
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdentifierSingleQuoted;
                    return false;
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_after_doctype_system_keyword(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => {
                    self.state = State::BeforeDoctypeSystemIdentifier;
                    return false;
                }
                Some('"') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierDoubleQuoted;
                    return false;
                }
                Some('\'') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierSingleQuoted;
                    return false;
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_before_doctype_public_identifier(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => continue,
                Some('"') => {
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdentifierDoubleQuoted;
                    return false;
                }
                Some('\'') => {
                    self.doctype_public = Some(String::new());
                    self.state = State::DoctypePublicIdentifierSingleQuoted;
                    return false;
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_doctype_public_identifier_double_quoted(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some('"') => {
                    self.state = State::AfterDoctypePublicIdentifier;
                    return false;
                }
                Some('\0') => {
                    if let Some(ref mut s) = self.doctype_public {
                        s.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(c) => {
                    if let Some(ref mut s) = self.doctype_public {
                        s.push(c);
                    }
                }
            }
        }
    }

    fn state_doctype_public_identifier_single_quoted(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some('\'') => {
                    self.state = State::AfterDoctypePublicIdentifier;
                    return false;
                }
                Some('\0') => {
                    if let Some(ref mut s) = self.doctype_public {
                        s.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(c) => {
                    if let Some(ref mut s) = self.doctype_public {
                        s.push(c);
                    }
                }
            }
        }
    }

    fn state_after_doctype_public_identifier(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => {
                    self.state = State::BetweenDoctypePublicAndSystemIdentifiers;
                    return false;
                }
                Some('>') => {
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some('"') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierDoubleQuoted;
                    return false;
                }
                Some('\'') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierSingleQuoted;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_between_doctype_public_and_system_identifiers(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => continue,
                Some('>') => {
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some('"') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierDoubleQuoted;
                    return false;
                }
                Some('\'') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierSingleQuoted;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_before_doctype_system_identifier(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => continue,
                Some('"') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierDoubleQuoted;
                    return false;
                }
                Some('\'') => {
                    self.doctype_system = Some(String::new());
                    self.state = State::DoctypeSystemIdentifierSingleQuoted;
                    return false;
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {
                    self.doctype_force_quirks = true;
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_doctype_system_identifier_double_quoted(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some('"') => {
                    self.state = State::AfterDoctypeSystemIdentifier;
                    return false;
                }
                Some('\0') => {
                    if let Some(ref mut s) = self.doctype_system {
                        s.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(c) => {
                    if let Some(ref mut s) = self.doctype_system {
                        s.push(c);
                    }
                }
            }
        }
    }

    fn state_doctype_system_identifier_single_quoted(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some('\'') => {
                    self.state = State::AfterDoctypeSystemIdentifier;
                    return false;
                }
                Some('\0') => {
                    if let Some(ref mut s) = self.doctype_system {
                        s.push('\u{FFFD}');
                    }
                }
                Some('>') => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(c) => {
                    if let Some(ref mut s) = self.doctype_system {
                        s.push(c);
                    }
                }
            }
        }
    }

    fn state_after_doctype_system_identifier(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.doctype_force_quirks = true;
                    self.emit_doctype();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(c) if is_whitespace(c) => continue,
                Some('>') => {
                    self.emit_doctype();
                    self.state = State::Data;
                    return false;
                }
                Some(_) => {
                    self.reconsume_current();
                    self.state = State::BogusDoctype;
                    return false;
                }
            }
        }
    }

    fn state_cdata_section(&mut self) -> bool {
        loop {
            match self.get_char() {
                None => {
                    self.flush_text();
                    self.tokens.push(Token::EOF);
                    return true;
                }
                Some(']') => {
                    self.state = State::CdataSectionBracket;
                    return false;
                }
                Some(c) => {
                    self.append_text(c);
                }
            }
        }
    }

    fn state_cdata_section_bracket(&mut self) -> bool {
        match self.get_char() {
            Some(']') => {
                self.state = State::CdataSectionEnd;
                false
            }
            None => {
                self.append_text(']');
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(_) => {
                self.append_text(']');
                self.reconsume_current();
                self.state = State::CdataSection;
                false
            }
        }
    }

    fn state_cdata_section_end(&mut self) -> bool {
        match self.get_char() {
            Some('>') => {
                self.flush_text();
                self.state = State::Data;
                false
            }
            Some(']') => {
                self.append_text(']');
                false
            }
            None => {
                self.text_buffer.push_str("]]");
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some(_) => {
                self.text_buffer.push_str("]]");
                self.reconsume_current();
                self.state = State::CdataSection;
                false
            }
        }
    }

    fn state_rcdata(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('<') => {
                self.state = State::RcdataLessThanSign;
                false
            }
            Some('\0') => {
                self.append_text('\u{FFFD}');
                false
            }
            Some(c) => {
                self.append_text(c);
                false
            }
        }
    }

    fn state_rcdata_less_than_sign(&mut self) -> bool {
        match self.get_char() {
            Some('/') => {
                self.current_tag_name.clear();
                self.original_tag_name.clear();
                self.state = State::RcdataEndTagOpen;
                false
            }
            _ => {
                self.append_text('<');
                self.reconsume_current();
                self.state = State::Rcdata;
                false
            }
        }
    }

    fn state_rcdata_end_tag_open(&mut self) -> bool {
        match self.get_char() {
            Some(c) if is_ascii_alpha(c) => {
                self.current_tag_name.push(ascii_lower(c));
                self.original_tag_name.push(c);
                self.state = State::RcdataEndTagName;
                false
            }
            _ => {
                self.text_buffer.push_str("</");
                self.reconsume_current();
                self.state = State::Rcdata;
                false
            }
        }
    }

    fn state_rcdata_end_tag_name(&mut self) -> bool {
        loop {
            match self.get_char() {
                Some(c) if is_ascii_alpha(c) => {
                    self.current_tag_name.push(ascii_lower(c));
                    self.original_tag_name.push(c);
                }
                Some(c) => {
                    let tag_name = self.current_tag_name.clone();
                    if Some(&tag_name) == self.rawtext_tag_name.as_ref() {
                        if c == '>' {
                            self.flush_text();
                            self.tokens.push(Token::EndTag { name: tag_name });
                            self.state = State::Data;
                            self.rawtext_tag_name = None;
                            self.current_tag_name.clear();
                            self.original_tag_name.clear();
                            return false;
                        }
                        if is_whitespace(c) {
                            self.flush_text();
                            self.is_end_tag = true;
                            self.current_tag_attrs.clear();
                            self.state = State::BeforeAttributeName;
                            return false;
                        }
                        if c == '/' {
                            self.flush_text();
                            self.is_end_tag = true;
                            self.current_tag_attrs.clear();
                            self.state = State::SelfClosingStartTag;
                            return false;
                        }
                    }

                    self.text_buffer.push_str("</");
                    self.text_buffer.push_str(&self.original_tag_name);
                    self.current_tag_name.clear();
                    self.original_tag_name.clear();
                    self.reconsume_current();
                    self.state = State::Rcdata;
                    return false;
                }
                None => {
                    self.text_buffer.push_str("</");
                    self.text_buffer.push_str(&self.original_tag_name);
                    self.current_tag_name.clear();
                    self.original_tag_name.clear();
                    self.flush_text();
                    self.tokens.push(Token::EOF);
                    return true;
                }
            }
        }
    }

    fn state_rawtext(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('\0') => {
                self.append_text('\u{FFFD}');
                false
            }
            Some('<') => {
                self.state = State::RawtextLessThanSign;
                false
            }
            Some(c) => {
                self.append_text(c);
                false
            }
        }
    }

    fn state_rawtext_less_than_sign(&mut self) -> bool {
        match self.get_char() {
            Some('/') => {
                self.current_tag_name.clear();
                self.original_tag_name.clear();
                self.state = State::RawtextEndTagOpen;
                false
            }
            _ => {
                self.append_text('<');
                self.reconsume_current();
                self.state = State::Rawtext;
                false
            }
        }
    }

    fn state_rawtext_end_tag_open(&mut self) -> bool {
        match self.get_char() {
            Some(c) if is_ascii_alpha(c) => {
                self.current_tag_name.push(ascii_lower(c));
                self.original_tag_name.push(c);
                self.state = State::RawtextEndTagName;
                false
            }
            _ => {
                self.text_buffer.push_str("</");
                self.reconsume_current();
                self.state = State::Rawtext;
                false
            }
        }
    }

    fn state_rawtext_end_tag_name(&mut self) -> bool {
        loop {
            match self.get_char() {
                Some(c) if is_ascii_alpha(c) => {
                    self.current_tag_name.push(ascii_lower(c));
                    self.original_tag_name.push(c);
                }
                Some(c) => {
                    let tag_name = self.current_tag_name.clone();
                    if Some(&tag_name) == self.rawtext_tag_name.as_ref() {
                        if c == '>' {
                            self.flush_text();
                            self.tokens.push(Token::EndTag { name: tag_name });
                            self.state = State::Data;
                            self.rawtext_tag_name = None;
                            self.current_tag_name.clear();
                            self.original_tag_name.clear();
                            return false;
                        }
                        if is_whitespace(c) {
                            self.flush_text();
                            self.is_end_tag = true;
                            self.current_tag_attrs.clear();
                            self.state = State::BeforeAttributeName;
                            return false;
                        }
                        if c == '/' {
                            self.flush_text();
                            self.is_end_tag = true;
                            self.current_tag_attrs.clear();
                            self.state = State::SelfClosingStartTag;
                            return false;
                        }
                    }

                    self.text_buffer.push_str("</");
                    self.text_buffer.push_str(&self.original_tag_name);
                    self.current_tag_name.clear();
                    self.original_tag_name.clear();
                    self.reconsume_current();
                    self.state = State::Rawtext;
                    return false;
                }
                None => {
                    self.text_buffer.push_str("</");
                    self.text_buffer.push_str(&self.original_tag_name);
                    self.current_tag_name.clear();
                    self.original_tag_name.clear();
                    self.flush_text();
                    self.tokens.push(Token::EOF);
                    return true;
                }
            }
        }
    }

    fn state_plaintext(&mut self) -> bool {
        match self.get_char() {
            None => {
                self.flush_text();
                self.tokens.push(Token::EOF);
                true
            }
            Some('\0') => {
                self.append_text('\u{FFFD}');
                false
            }
            Some(c) => {
                self.append_text(c);
                false
            }
        }
    }
}

// WASM bindings

#[wasm_bindgen]
pub struct WasmTokenizer {
    inner: Tokenizer,
}

#[wasm_bindgen]
impl WasmTokenizer {
    #[wasm_bindgen(constructor)]
    pub fn new(html: &str) -> WasmTokenizer {
        WasmTokenizer {
            inner: Tokenizer::new(html),
        }
    }

    /// Tokenize the entire input and return tokens as JSON
    #[wasm_bindgen]
    pub fn tokenize(&mut self) -> JsValue {
        let tokens = self.inner.tokenize();
        serde_wasm_bindgen::to_value(tokens).unwrap_or(JsValue::NULL)
    }

    /// Count tokens without returning them (for benchmarking)
    #[wasm_bindgen]
    pub fn count_tokens(&mut self) -> usize {
        self.inner.tokenize().len()
    }
}

/// Tokenize HTML and return tokens as JSON (simple wrapper function)
#[wasm_bindgen]
pub fn tokenize_html(html: &str) -> JsValue {
    let mut tokenizer = Tokenizer::new(html);
    let tokens = tokenizer.tokenize();
    serde_wasm_bindgen::to_value(tokens).unwrap_or(JsValue::NULL)
}

/// Count tokens without returning them (for benchmarking)
#[wasm_bindgen]
pub fn count_tokens(html: &str) -> usize {
    let mut tokenizer = Tokenizer::new(html);
    tokenizer.tokenize().len()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simple_html() {
        let mut tokenizer = Tokenizer::new("<p>Hello</p>");
        let tokens = tokenizer.tokenize();
        assert_eq!(tokens.len(), 4); // start tag, text, end tag, EOF
    }

    #[test]
    fn test_attributes() {
        let mut tokenizer = Tokenizer::new("<div class=\"test\" id='foo'>content</div>");
        let tokens = tokenizer.tokenize();

        if let Token::StartTag { name, attrs, .. } = &tokens[0] {
            assert_eq!(name, "div");
            assert!(attrs.iter().any(|(k, v)| k == "class" && v == "test"));
            assert!(attrs.iter().any(|(k, v)| k == "id" && v == "foo"));
        } else {
            panic!("Expected StartTag");
        }
    }

    #[test]
    fn test_comment() {
        let mut tokenizer = Tokenizer::new("<!-- this is a comment -->");
        let tokens = tokenizer.tokenize();

        if let Token::Comment { data } = &tokens[0] {
            assert_eq!(data, " this is a comment ");
        } else {
            panic!("Expected Comment");
        }
    }

    #[test]
    fn test_doctype() {
        let mut tokenizer = Tokenizer::new("<!DOCTYPE html>");
        let tokens = tokenizer.tokenize();

        if let Token::Doctype { name, .. } = &tokens[0] {
            assert_eq!(name.as_deref(), Some("html"));
        } else {
            panic!("Expected Doctype");
        }
    }
}
