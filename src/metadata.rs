use colored::Colorize;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::process::exit;

#[derive(Deserialize, Debug, Serialize, Default)]
/// Represents the raw metadata structure parsed from a JSON file.
///
/// This struct is used as an intermediate representation of metadata
/// before it is converted into the `Metadata` struct.
struct RawMetadata {
    title: String,
    subtitle: Option<String>,
    series: Option<Vec<String>>,
    authors: Option<Vec<String>>,
    published_year: Option<String>,
    published_date: Option<String>,
    genres: Option<Vec<String>>,
    language: Option<String>,
    abridged: Option<bool>,
}

/// Represents the processed metadata for an audiobook.
///
/// This struct contains detailed information about an audiobook, including
/// its title, author, series, and other attributes. It is derived from
/// the `RawMetadata` struct.
#[derive(Debug, Default, Serialize)]
pub struct Metadata {
    pub title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub series: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_number: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub book_number_with_zeros: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub published_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub abridged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_number: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_number_with_zeros: Option<String>,
}

/**
 * Parses metadata from a JSON file and converts it into a `Metadata` object.
 *
 * @param path The file path to the JSON metadata file.
 * @return An `Option` containing the parsed `Metadata` object, or `None` if parsing fails.
 */
pub fn parse_metadata(path: &str) -> Option<Metadata> {
    let file_contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(e) => {
            eprintln!(
                "{} '{}'. {}",
                "Error: Could not read the file".red(),
                path.yellow(),
                e
            );
            exit(1);
        }
    };

    match serde_json::from_str::<RawMetadata>(&file_contents) {
        Ok(raw_data) => {
            println!("Successfully parsed metadata file '{}'", path);

            let author = raw_data
                .authors
                .and_then(|authors| authors.first().cloned());
            let genre = raw_data.genres.and_then(|genres| genres.first().cloned());
            let full_series = raw_data.series.and_then(|series| series.first().cloned());
            let (series, book_number) = match full_series {
                Some(s) => {
                    let re = Regex::new(r"^(.+)\s+#?(\d+)$").unwrap();
                    if let Some(results) = re.captures(&s) {
                        let series = Some(results[1].to_string());
                        let book_number = results[2].parse::<u16>().ok();
                        (series, book_number)
                    } else {
                        (None, None)
                    }
                }
                None => (None, None),
            };

            Some(Metadata {
                title: raw_data.title,
                subtitle: raw_data.subtitle,
                series,
                book_number,
                book_number_with_zeros: None,
                author,
                published_year: raw_data.published_year,
                published_date: raw_data.published_date,
                genre,
                language: raw_data.language,
                abridged: raw_data.abridged,
                file_number: None,
                file_number_with_zeros: None,
            })
        }
        Err(_) => {
            eprintln!("{} '{}'", "Error: Failed to parse file".red(), path);
            None
        }
    }
}
