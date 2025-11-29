use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use regex::Regex;

/**
 * Get the track number from a file's metadata.
 *
 * This function attempts to extract the track number from the file's metadata.
 * If the track number is not found or is invalid, it returns None.
 */
pub fn get_track_number(path: &str) -> Option<u16> {
    // 1. Try to read internal metadata (ID3, etc.)
    //    Probe::open checks the file extension and content to figure out the format.
    //    We return Result or Option at every step to ensure safe fallthrough.
    if let Ok(tagged_file) = Probe::open(path).and_then(|p| p.read()) {
        if let Some(tag) = tagged_file.primary_tag() {
            if let Some(track) = tag.track() {
                // Some files might have a tag set to 0, which is usually invalid.
                // We treat 0 as "missing" so we fall back to filename parsing.
                if track > 0 {
                    return Some(track as u16);
                }
            }
        }
    }

    // 2. Fallback: If no internal tag (or track was 0), parse the filename
    //    This part runs if ANY step above fails or returns None.
    return parse_from_filename(path);
}

/**
 * Extracts the file number from a file name.
 *
 * The file number is typically a whole number found at the start of the file name.
 * If multiple numbers are present, context-based rules are applied to determine the correct one.
 *
 * @param file_name The name of the file to analyze.
 * @return An `Option<u16>` containing the extracted file number, or `None` if no valid number is found.
 */
fn parse_from_filename(file_name: &str) -> Option<u16> {
    // We will collect numbers to IGNORE here.
    let mut ignore_list: Vec<u16> = Vec::new();

    // 1. Identify "Book" number to ignore (e.g., "Book 3")
    let re_book = Regex::new(r"(?i)\bbook\s*#?\s*(\d+)\b").unwrap();
    if let Some(caps) = re_book.captures(file_name) {
        if let Ok(num) = caps[1].parse::<u16>() {
            ignore_list.push(num);
        }
    }

    // 2. Identify Dates (YYYY-MM-DD) to ignore
    let re_date_iso = Regex::new(r"\b(\d{4})[-/.](\d{1,2})[-/.](\d{1,2})\b").unwrap();
    for caps in re_date_iso.captures_iter(file_name) {
        if let Ok(y) = caps[1].parse::<u16>() {
            ignore_list.push(y);
        }
        if let Ok(m) = caps[2].parse::<u16>() {
            ignore_list.push(m);
        }
        if let Ok(d) = caps[3].parse::<u16>() {
            ignore_list.push(d);
        }
    }

    // 3. Identify Dates (MM/DD/YYYY or DD.MM.YYYY) to ignore
    let re_date_common = Regex::new(r"\b(\d{1,2})[-/.](\d{1,2})[-/.](\d{4})\b").unwrap();
    for caps in re_date_common.captures_iter(file_name) {
        if let Ok(d1) = caps[1].parse::<u16>() {
            ignore_list.push(d1);
        }
        if let Ok(d2) = caps[2].parse::<u16>() {
            ignore_list.push(d2);
        }
        if let Ok(y) = caps[3].parse::<u16>() {
            ignore_list.push(y);
        }
    }

    // 4. Identify Short Dates (MM/DD/YY or DD.MM.YY) to ignore
    //    We strictly look for 2 digits at the end to catch "11/27/25"
    let re_date_short = Regex::new(r"\b(\d{1,2})[-/.](\d{1,2})[-/.](\d{2})\b").unwrap();
    for caps in re_date_short.captures_iter(file_name) {
        if let Ok(d1) = caps[1].parse::<u16>() {
            ignore_list.push(d1);
        }
        if let Ok(d2) = caps[2].parse::<u16>() {
            ignore_list.push(d2);
        }
        if let Ok(y) = caps[3].parse::<u16>() {
            ignore_list.push(y);
        }
    }

    // 5. Explicit Context (Section, Chapter, Part, Track) - Highest Priority
    let re_context = Regex::new(r"(?i)\b(section|chapter|part|track)\s*#?\s*(\d+)\b").unwrap();
    if let Some(caps) = re_context.captures(file_name) {
        return caps[2].parse().ok();
    }

    // 6. "X of Y" Pattern (e.g. "2 of 13")
    let re_of = Regex::new(r"(?i)\b(\d+)\s*of\s*\d+").unwrap();
    if let Some(caps) = re_of.captures(file_name) {
        let num = caps[1].parse().ok();
        if let Some(n) = num {
            if !ignore_list.contains(&n) {
                return Some(n);
            }
        }
    }

    // 7. Start Pattern (e.g. "02 -", "01. Song", "BH_19-")
    //    Modified to include `.` in separator class `[-_.]` to handle "01. Title"
    let re_start = Regex::new(r"^(?:[a-zA-Z]+[_\s-]*)?(\d{1,3})\s*[-_.]").unwrap();
    if let Some(caps) = re_start.captures(file_name) {
        let num = caps[1].parse().ok();
        if let Some(n) = num {
            if !ignore_list.contains(&n) {
                return Some(n);
            }
        }
    }

    // 8. Track-Total Pattern anywhere (e.g. "19-37", "01/12")
    let re_track_total = Regex::new(r"\b(\d{1,3})[-/_]\d+\b").unwrap();
    if let Some(caps) = re_track_total.captures(file_name) {
        let num = caps[1].parse().ok();
        if let Some(n) = num {
            if !ignore_list.contains(&n) {
                return Some(n);
            }
        }
    }

    // 9. Delimited Suffix (e.g. "- 02", "_2", "_02")
    let re_suffix = Regex::new(r"[-_]\s*(\d+)$").unwrap();
    if let Some(caps) = re_suffix.captures(file_name) {
        let num = caps[1].parse().ok();
        if let Some(n) = num {
            if !ignore_list.contains(&n) {
                return Some(n);
            }
        }
    }

    // 10. Solo Number Pattern (e.g. "02", "2")
    //    Only accept if the ENTIRE string is just the number.
    let re_solo = Regex::new(r"^\s*(\d+)\s*$").unwrap();
    if let Some(caps) = re_solo.captures(file_name) {
        let num = caps[1].parse().ok();
        if let Some(n) = num {
            if !ignore_list.contains(&n) {
                return Some(n);
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_from_filename() {
        // Tuple format: (input_filename, expected_track_number)
        let inputs = [
            ("02 - book title", Some(2)),
            ("02 - book with number 3 in title", Some(2)),
            ("2 - book with title - book 3", Some(2)),
            ("Book 3 - title - 02", Some(2)),
            ("Book3 - title - 2", Some(2)),
            ("Book 3 - title_2", Some(2)),
            ("Book3 - title with number 4 in it - 2 of 13", Some(2)),
            ("book 3 - title - 2of13", Some(2)),
            ("Author - Title with number 4 in it", None),
            ("Book 3 - title", None),
            ("Title with number 4 in it", None),
            ("Book 3 - section 7 - title", Some(7)),
            ("Book3 - section7 - title", Some(7)),
            ("Book 3 - title - section 7", Some(7)),
            ("BH_19-37 title", Some(19)),
            ("19-37 title", Some(19)),
            ("author - title - 19-37", Some(19)),
            ("The Lady of the Camellias_MP3WRAP", None),
            ("author - title 2025-11-27 with date", None),
            ("author - title 11-27-2025 with date", None),
            ("author - title 11/27/2025 with date", None),
            ("author - title 11/27/25 with date", None),
            ("author - title 11.27.2025 with date", None),
        ];

        for (input, expected) in inputs {
            let result = parse_from_filename(input);
            assert_eq!(
                result, expected,
                "Failed on input: '{}'. Expected {:?}, got {:?}",
                input, expected, result
            );
        }
    }
}
