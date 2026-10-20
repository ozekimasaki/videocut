//! Import and export VideoCut sequences as OpenTimelineIO JSON, FCPXML 1.9,
//! and CMX 3600 EDL.
//!
//! Clip edges in a VideoCut sequence abut at each cut. A transition is stored
//! on the track with a center, a duration, and an alignment. OTIO keeps that
//! shape: the transition's `in_offset` and `out_offset` reach into the
//! neighboring clips. FCPXML and CMX overlap the record times by the blend
//! and this crate trims those handles back on import.

mod assemble;
mod edit;
mod edl;
mod error;
mod fcpxml;
mod names;
mod otio;
mod timeutil;

use videocut_model::Project;

pub use error::Error;

/// A project read from an interchange document, plus notes about objects that
/// could not be represented exactly.
#[derive(Debug, Clone, PartialEq)]
pub struct ImportedProject {
    pub project: Project,
    pub warnings: Vec<String>,
}

/// Text written for an interchange format, plus notes about omissions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportedDocument {
    pub text: String,
    pub warnings: Vec<String>,
}

/// Read an OpenTimelineIO JSON timeline or collection into a project.
pub fn import_otio(text: &str) -> Result<ImportedProject, Error> {
    let (draft, mut warnings) = otio::import(text)?;
    warnings.extend(draft.warnings.iter().cloned());
    Ok(ImportedProject {
        project: assemble::realize(draft),
        warnings,
    })
}

/// Write every sequence in `project` as OpenTimelineIO JSON.
pub fn export_otio(project: &Project) -> Result<ExportedDocument, Error> {
    let (text, warnings) = otio::export(project)?;
    Ok(ExportedDocument { text, warnings })
}

/// Read FCPXML 1.9 or newer. Older 1.x documents are read with a warning.
pub fn import_fcpxml(text: &str) -> Result<ImportedProject, Error> {
    let (draft, mut warnings) = fcpxml::import(text)?;
    warnings.extend(draft.warnings.iter().cloned());
    Ok(ImportedProject {
        project: assemble::realize(draft),
        warnings,
    })
}

/// Write every sequence in `project` as FCPXML 1.9.
pub fn export_fcpxml(project: &Project) -> Result<ExportedDocument, Error> {
    let (text, warnings) = fcpxml::export(project)?;
    Ok(ExportedDocument { text, warnings })
}

/// Read a CMX 3600 EDL into a project with one sequence.
pub fn import_edl(text: &str) -> Result<ImportedProject, Error> {
    let (draft, mut warnings) = edl::import(text)?;
    warnings.extend(draft.warnings.iter().cloned());
    Ok(ImportedProject {
        project: assemble::realize(draft),
        warnings,
    })
}

/// Write the first sequence in `project` as a CMX 3600 EDL.
pub fn export_edl(project: &Project) -> Result<ExportedDocument, Error> {
    let (text, warnings) = edl::export(project)?;
    Ok(ExportedDocument { text, warnings })
}

/// Write one sequence as a CMX 3600 EDL.
pub fn export_edl_sequence(
    project: &Project,
    sequence_id: &str,
) -> Result<ExportedDocument, Error> {
    let (text, warnings) = edl::export_sequence(project, sequence_id)?;
    Ok(ExportedDocument { text, warnings })
}
