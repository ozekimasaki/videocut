//! XML document tree and gzip framing for `.prproj` payloads.

use std::io::{Read, Write};

use flate2::read::GzDecoder;
use flate2::write::GzEncoder;
use flate2::Compression;
use quick_xml::events::{BytesDecl, BytesEnd, BytesStart, BytesText, Event};
use quick_xml::Reader;
use quick_xml::Writer;

const GZIP_MAGIC: &[u8] = b"\x1f\x8b";

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PrprojError {
    #[error("project XML is not well formed: {0}")]
    Xml(String),
    #[error("gzip payload could not be read: {0}")]
    Gzip(String),
    #[error("file is not gzip-compressed project XML")]
    NotProject,
    #[error("frame rate must be a positive rational")]
    FrameRate,
    #[error("{0}")]
    Io(String),
}

impl From<std::io::Error> for PrprojError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value.to_string())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlElement {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XmlNode>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum XmlNode {
    Element(XmlElement),
    Text(String),
    Comment(String),
}

impl XmlElement {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            attrs: Vec::new(),
            children: Vec::new(),
        }
    }

    pub fn attr(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.attrs.push((key.into(), value.into()));
        self
    }

    pub fn child(mut self, child: XmlElement) -> Self {
        self.children.push(XmlNode::Element(child));
        self
    }

    pub fn text_child(self, name: impl Into<String>, text: impl Into<String>) -> Self {
        self.child(XmlElement::new(name).with_text(text))
    }

    pub fn with_text(mut self, text: impl Into<String>) -> Self {
        self.children.push(XmlNode::Text(text.into()));
        self
    }

    pub fn get_attr(&self, key: &str) -> Option<&str> {
        self.attrs
            .iter()
            .find(|(name, _)| name == key)
            .map(|(_, value)| value.as_str())
    }

    pub fn child_named(&self, name: &str) -> Option<&XmlElement> {
        self.children.iter().find_map(|node| match node {
            XmlNode::Element(element) if element.name == name => Some(element),
            _ => None,
        })
    }

    pub fn children_named<'a>(&'a self, name: &'a str) -> impl Iterator<Item = &'a XmlElement> {
        self.children.iter().filter_map(move |node| match node {
            XmlNode::Element(element) if element.name == name => Some(element),
            _ => None,
        })
    }

    pub fn text(&self) -> Option<String> {
        let mut combined = String::new();
        for node in &self.children {
            if let XmlNode::Text(text) = node {
                combined.push_str(text);
            }
        }
        let trimmed = combined.trim();
        if trimmed.is_empty() {
            None
        } else {
            Some(trimmed.to_string())
        }
    }

    pub fn child_text(&self, name: &str) -> Option<String> {
        self.child_named(name).and_then(XmlElement::text)
    }

    pub fn descend(&self, path: &[&str]) -> Option<&XmlElement> {
        let mut current = self;
        for name in path {
            current = current.child_named(name)?;
        }
        Some(current)
    }

    pub fn path_text(&self, path: &[&str]) -> Option<String> {
        self.descend(path).and_then(XmlElement::text)
    }

    pub fn object_id(&self) -> Option<&str> {
        self.get_attr("ObjectID")
            .or_else(|| self.get_attr("ObjectUID"))
    }
}

pub fn parse_xml(xml: &str) -> Result<XmlElement, PrprojError> {
    let xml = xml.trim_start_matches('\u{feff}');
    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(false);
    let mut buf = Vec::new();
    let mut stack: Vec<XmlElement> = Vec::new();
    let mut root: Option<XmlElement> = None;

    loop {
        let event = reader
            .read_event_into(&mut buf)
            .map_err(|err| PrprojError::Xml(err.to_string()))?;
        match event {
            Event::Start(start) => {
                stack.push(element_from_start(&start)?);
            }
            Event::Empty(start) => {
                let element = element_from_start(&start)?;
                push_element(&mut stack, &mut root, element)?;
            }
            Event::End(_) => {
                let element = stack
                    .pop()
                    .ok_or_else(|| PrprojError::Xml("unbalanced end tag".into()))?;
                push_element(&mut stack, &mut root, element)?;
            }
            Event::Text(text) => {
                let value = text
                    .unescape()
                    .map_err(|err| PrprojError::Xml(err.to_string()))?
                    .into_owned();
                if value.chars().any(|ch| !ch.is_whitespace()) {
                    if let Some(parent) = stack.last_mut() {
                        parent.children.push(XmlNode::Text(value));
                    }
                }
            }
            Event::CData(text) => {
                let value = String::from_utf8_lossy(&text).into_owned();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Text(value));
                }
            }
            Event::Comment(comment) => {
                let value = String::from_utf8_lossy(&comment).into_owned();
                if let Some(parent) = stack.last_mut() {
                    parent.children.push(XmlNode::Comment(value));
                }
            }
            Event::Eof => break,
            Event::Decl(_) | Event::PI(_) | Event::DocType(_) => {}
        }
        buf.clear();
    }

    if !stack.is_empty() {
        return Err(PrprojError::Xml("unclosed element".into()));
    }
    root.ok_or_else(|| PrprojError::Xml("empty document".into()))
}

fn element_from_start(start: &BytesStart<'_>) -> Result<XmlElement, PrprojError> {
    let name = String::from_utf8_lossy(start.name().as_ref()).into_owned();
    let mut element = XmlElement::new(name);
    for attr in start.attributes() {
        let attr = attr.map_err(|err| PrprojError::Xml(err.to_string()))?;
        let key = String::from_utf8_lossy(attr.key.as_ref()).into_owned();
        let value = attr
            .unescape_value()
            .map_err(|err| PrprojError::Xml(err.to_string()))?
            .into_owned();
        element.attrs.push((key, value));
    }
    Ok(element)
}

fn push_element(
    stack: &mut [XmlElement],
    root: &mut Option<XmlElement>,
    element: XmlElement,
) -> Result<(), PrprojError> {
    if let Some(parent) = stack.last_mut() {
        parent.children.push(XmlNode::Element(element));
        Ok(())
    } else if root.is_none() {
        *root = Some(element);
        Ok(())
    } else {
        Err(PrprojError::Xml(
            "document has more than one root element".into(),
        ))
    }
}

pub fn write_xml(root: &XmlElement) -> String {
    let mut writer = Writer::new_with_indent(Vec::new(), b'\t', 1);
    let decl = BytesDecl::new("1.0", Some("UTF-8"), None);
    writer
        .write_event(Event::Decl(decl))
        .expect("xml declaration writes into a buffer");
    write_element(&mut writer, root);
    let bytes = writer.into_inner();
    let mut text = String::from_utf8(bytes).expect("xml writer emits utf-8");
    if !text.ends_with('\n') {
        text.push('\n');
    }
    text
}

fn write_element(writer: &mut Writer<Vec<u8>>, element: &XmlElement) {
    let mut start = BytesStart::new(element.name.as_str());
    for (key, value) in &element.attrs {
        start.push_attribute((key.as_str(), value.as_str()));
    }
    let has_children = element
        .children
        .iter()
        .any(|node| !matches!(node, XmlNode::Text(text) if text.trim().is_empty()));
    if !has_children {
        writer
            .write_event(Event::Empty(start))
            .expect("xml element writes into a buffer");
        return;
    }
    writer
        .write_event(Event::Start(start))
        .expect("xml element writes into a buffer");
    for child in &element.children {
        match child {
            XmlNode::Element(child) => write_element(writer, child),
            XmlNode::Text(text) => {
                writer
                    .write_event(Event::Text(BytesText::new(text)))
                    .expect("xml text writes into a buffer");
            }
            XmlNode::Comment(text) => {
                writer
                    .write_event(Event::Comment(BytesText::new(text)))
                    .expect("xml comment writes into a buffer");
            }
        }
    }
    writer
        .write_event(Event::End(BytesEnd::new(element.name.as_str())))
        .expect("xml end tag writes into a buffer");
}

pub fn decode_payload(bytes: &[u8]) -> Result<Vec<u8>, PrprojError> {
    if bytes.starts_with(GZIP_MAGIC) {
        let mut decoder = GzDecoder::new(bytes);
        let mut xml = Vec::new();
        decoder
            .read_to_end(&mut xml)
            .map_err(|err| PrprojError::Gzip(err.to_string()))?;
        Ok(xml)
    } else if bytes.windows(5).any(|window| window == b"<?xml")
        || bytes.windows(13).any(|window| window == b"<PremiereData")
    {
        Ok(bytes.to_vec())
    } else {
        Err(PrprojError::NotProject)
    }
}

pub fn encode_gzip(xml: &str) -> Result<Vec<u8>, PrprojError> {
    let mut encoder = GzEncoder::new(Vec::new(), Compression::new(6));
    encoder.write_all(xml.as_bytes())?;
    Ok(encoder.finish()?)
}
