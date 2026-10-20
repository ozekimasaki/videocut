//! Read and write gzip-compressed `.prproj` XML.
//!
//! The document is an object graph: top-level elements carry `ObjectID` or
//! `ObjectUID`, and other elements point at them with `ObjectRef` or
//! `ObjectURef`. This crate maps bins, sequences, frame size, frame rate,
//! video and audio tracks, clip and source ranges, markers, speed, links,
//! opacity, motion, and cross dissolves to [`videocut_model::Project`].
//!
//! Objects this crate does not model stay in the document tree and are listed
//! in [`PrprojDocument::warnings`]. Saving the document writes them back.

mod read;
mod write;
mod xml;

use std::fs;
use std::path::Path;

use videocut_model::Project;

pub use xml::PrprojError;

/// Something in the file that was kept but not fully represented on [`Project`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrprojWarning {
    pub object_id: Option<String>,
    pub tag: String,
    pub message: String,
}

/// A parsed `.prproj` document and the [`Project`] mapped from it.
#[derive(Debug, Clone)]
pub struct PrprojDocument {
    root: xml::XmlElement,
    project: Project,
    warnings: Vec<PrprojWarning>,
}

impl PrprojDocument {
    /// Build a document that contains only the objects [`Project`] maps to.
    pub fn from_project(project: &Project) -> Result<Self, PrprojError> {
        let root = write::project_to_xml(project)?;
        let (mapped, warnings) = read::project_from_xml(&root);
        Ok(Self {
            root,
            project: mapped,
            warnings,
        })
    }

    /// Parse uncompressed project XML.
    pub fn read_xml(xml: &str) -> Result<Self, PrprojError> {
        let root = xml::parse_xml(xml)?;
        if root.name != "PremiereData" {
            return Err(PrprojError::NotProject);
        }
        let (project, warnings) = read::project_from_xml(&root);
        Ok(Self {
            root,
            project,
            warnings,
        })
    }

    /// Parse a `.prproj` payload. Gzip framing is removed when the gzip magic
    /// is present; plain XML is accepted as well.
    pub fn read_bytes(bytes: &[u8]) -> Result<Self, PrprojError> {
        let xml = xml::decode_payload(bytes)?;
        let text = String::from_utf8(xml).map_err(|err| PrprojError::Xml(err.to_string()))?;
        Self::read_xml(&text)
    }

    pub fn read_path(path: &Path) -> Result<Self, PrprojError> {
        let bytes = fs::read(path)?;
        Self::read_bytes(&bytes)
    }

    pub fn project(&self) -> &Project {
        &self.project
    }

    pub fn warnings(&self) -> &[PrprojWarning] {
        &self.warnings
    }

    /// XML text, including objects that were not mapped onto [`Project`].
    pub fn to_xml(&self) -> String {
        xml::write_xml(&self.root)
    }

    /// Gzip-compressed `.prproj` bytes.
    pub fn to_bytes(&self) -> Result<Vec<u8>, PrprojError> {
        xml::encode_gzip(&self.to_xml())
    }

    pub fn write_path(&self, path: &Path) -> Result<(), PrprojError> {
        fs::write(path, self.to_bytes()?)?;
        Ok(())
    }
}

/// Read gzip or plain XML and return the mapped project plus warnings.
pub fn read_prproj(bytes: &[u8]) -> Result<(Project, Vec<PrprojWarning>), PrprojError> {
    let doc = PrprojDocument::read_bytes(bytes)?;
    Ok((doc.project, doc.warnings))
}

/// Write a project to gzip-compressed `.prproj` bytes.
pub fn write_prproj(project: &Project) -> Result<Vec<u8>, PrprojError> {
    PrprojDocument::from_project(project)?.to_bytes()
}
