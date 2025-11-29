use crate::metadata::Metadata;
use crate::track::get_track_number;
use handlebars::{Handlebars, RenderError, no_escape};
use std::path::PathBuf;

/// Represents the schema used for formatting file paths and names.
///
/// This struct contains templates for generating directory paths and file names
/// based on metadata.
#[derive(Debug)]
pub struct Schema {
    pub path_template: String,
    pub file_template: String,
}

impl Schema {
    pub fn new(path: String, file: String) -> Self {
        Schema {
            path_template: path,
            file_template: file,
        }
    }

    /**
     * Formats a directory path based on the provided schema and metadata.
     *
     * @param metadata The metadata object containing information for formatting.
     * @return A `Result` containing the formatted path as a `String` or a `RenderError`.
     */
    pub fn fmt_path(&self, metadata: &mut Metadata) -> Result<String, RenderError> {
        let mut reg = Handlebars::new();
        reg.register_escape_fn(no_escape);
        metadata.book_number_with_zeros = metadata.book_number.map(|num| format!("{:02}", num));
        reg.register_template_string("path", &self.path_template)
            .unwrap();
        reg.set_strict_mode(true);
        reg.render("path", metadata)
    }

    /**
     * Formats a file name based on the provided schema, metadata, and file path.
     *
     * @param metadata A mutable reference to the metadata object for formatting.
     * @param file_path The path of the file to format.
     * @param file_ext A vector of allowed file extensions.
     * @return A `Result` containing the formatted file name as a `String` or a `RenderError`.
     */
    pub fn fmt_file(
        &self,
        metadata: &mut Metadata,
        file_path: &PathBuf,
        file_ext: &Vec<String>,
    ) -> Result<String, RenderError> {
        let mut reg = Handlebars::new();
        reg.register_escape_fn(no_escape);
        let full_file_name = file_path.file_name().unwrap().to_str().unwrap();
        let file_name = file_path.file_stem().unwrap().to_str().unwrap();
        let extension = file_path.extension().unwrap().to_str().unwrap();
        if file_ext.contains(&extension.to_string()) {
            let file_number = get_track_number(&file_name);
            metadata.file_number = file_number;
            metadata.file_number_with_zeros = file_number.map(|num| format!("{:03}", num));
            reg.register_template_string("file", &self.file_template)
                .unwrap();
            reg.set_strict_mode(true);
            return Ok(format!(
                "{}.{}",
                reg.render("file", metadata).unwrap(),
                extension
            ));
        }

        Ok(full_file_name.to_string())
    }
}
