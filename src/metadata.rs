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
    pub book_number: Option<String>,
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
 * Load the metadata file and then parse it
 *
 * @param path The file path to the JSON metadata file.
 * @return An `Option` containing the parsed `Metadata` object, or `None` if parsing fails.
 */
pub fn load_metadata(path: &str) -> Option<Metadata> {
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
            println!("Successfully loaded metadata file '{}'", path);
            parse_metadata(raw_data)
        }
        Err(_) => {
            eprintln!("{} '{}'", "Error: Failed to parse file".red(), path);
            None
        }
    }
}

/**
 * Parses metadata from a JSON file and converts it into a `Metadata` object.
 *
 * @param raw_data The raw metadata data.
 * @return An `Option` containing the parsed `Metadata` object, or `None` if parsing fails.
 */
fn parse_metadata(raw_data: RawMetadata) -> Option<Metadata> {
    let author = raw_data
        .authors
        .and_then(|authors| authors.first().cloned());
    let genre = raw_data.genres.and_then(|genres| genres.first().cloned());
    let full_series = raw_data.series.and_then(|series| series.first().cloned());
    let (series, book_number) = match full_series {
        Some(s) => {
            let re = Regex::new(r"^(.+)\s+#?(\d+\.\d+|\d+)$").unwrap();
            if let Some(results) = re.captures(&s) {
                let series = Some(results[1].to_string());
                let book_number_raw = results[2].parse::<f32>().ok();
                let book_number = Some(book_number_raw.unwrap().to_string());
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_metadata_basic() {
        let raw = RawMetadata {
            title: "Test Title".to_string(),
            subtitle: Some("Test Subtitle".to_string()),
            series: None,
            authors: Some(vec!["Author Name".to_string()]),
            published_year: Some("2024".to_string()),
            published_date: Some("2024-06-01".to_string()),
            genres: Some(vec!["Fiction".to_string()]),
            language: Some("English".to_string()),
            abridged: Some(false),
        };
        let meta = parse_metadata(raw).unwrap();
        assert_eq!(meta.title, "Test Title");
        assert_eq!(meta.subtitle, Some("Test Subtitle".to_string()));
        assert_eq!(meta.series, None);
        assert_eq!(meta.author, Some("Author Name".to_string()));
        assert_eq!(meta.published_year, Some("2024".to_string()));
        assert_eq!(meta.published_date, Some("2024-06-01".to_string()));
        assert_eq!(meta.genre, Some("Fiction".to_string()));
        assert_eq!(meta.language, Some("English".to_string()));
        assert_eq!(meta.abridged, Some(false));
    }

    #[test]
    fn test_parse_metadata_series_with_number() {
        let raw = RawMetadata {
            title: "Book Title".to_string(),
            subtitle: None,
            series: Some(vec!["Series Name #3".to_string()]),
            authors: Some(vec!["Author".to_string()]),
            published_year: None,
            published_date: None,
            genres: None,
            language: None,
            abridged: None,
        };
        let meta = parse_metadata(raw).unwrap();
        assert_eq!(meta.series, Some("Series Name".to_string()));
        assert_eq!(meta.book_number, Some("3".to_string()));
    }

    #[test]
    fn test_parse_metadata_series_without_number() {
        let raw = RawMetadata {
            title: "Book Title".to_string(),
            subtitle: None,
            series: Some(vec!["Series Name".to_string()]),
            authors: None,
            published_year: None,
            published_date: None,
            genres: None,
            language: None,
            abridged: None,
        };
        let meta = parse_metadata(raw).unwrap();
        assert_eq!(meta.series, None);
        assert_eq!(meta.book_number, None);
    }

    #[test]
    fn test_parse_metadata_missing_optional_fields() {
        let raw = RawMetadata {
            title: "Title".to_string(),
            subtitle: None,
            series: None,
            authors: None,
            published_year: None,
            published_date: None,
            genres: None,
            language: None,
            abridged: None,
        };
        let meta = parse_metadata(raw).unwrap();
        assert_eq!(meta.title, "Title");
        assert_eq!(meta.author, None);
        assert_eq!(meta.genre, None);
    }

    #[test]
    fn test_parse_metadata_series_with_float() {
        let raw = RawMetadata {
            title: "Book Title".to_string(),
            subtitle: None,
            series: Some(vec!["Series Name #6.4".to_string()]),
            authors: Some(vec!["Author".to_string()]),
            published_year: None,
            published_date: None,
            genres: None,
            language: None,
            abridged: None,
        };
        let meta = parse_metadata(raw).unwrap();
        assert_eq!(meta.series, Some("Series Name".to_string()));
        assert_eq!(meta.book_number, Some("6.4".to_string()));
    }
}
