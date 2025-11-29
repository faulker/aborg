use clap::Parser;
use colored::Colorize;
use handlebars::{Handlebars, RenderError, no_escape};
use lofty::file::TaggedFileExt;
use lofty::probe::Probe;
use lofty::tag::Accessor;
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::OnceLock;
use walkdir::WalkDir;

// TODO:
// - Add a "results" output at the end that prints total files touched, etc, also have it output a list of any errors
// - Fix the bug where the source dir is not being deleted when empty
// - Add chapter filtering from file name
// - Fix the file number bug when there is a date in the file title

static CONFIG: OnceLock<Config> = OnceLock::new();

#[derive(Parser, Debug)]
#[command(version, about, long_about = None)]
/// Represents the command-line arguments for the application.
///
/// This struct is used to parse and store the arguments provided by the user,
/// including source and destination directories, schemas, and other options.
struct Args {
    /// The directory containing the audiobook files you want to manage.
    /// This is the source directory for the operation.
    #[arg(short, long)]
    source: String,

    /// The directory` where the managed files will be moved.
    /// This is the destination directory for the operation.
    #[arg(short, long)]
    destination: String,

    /// The schema used to format the newly created destination directories.
    /// This uses the Handlebar schema style.
    #[arg(short, long, default_value_t = String::from("{{author}}/{{#if series}}{{series}}/{{/if}}{{title}}{{#if book_number_with_zeros}} - Book {{book_number_with_zeros}}{{/if}}"))]
    path_schema: String,

    /// The schema used to format the files that are being moved.
    /// This uses the Handlebar schema style.
    #[arg(short, long, default_value_t = String::from("{{#if series}}{{series}} - {{/if}}{{title}}{{#if file_number_with_zeros}} ({{file_number_with_zeros}}){{/if}}"))]
    file_schema: String,

    /// If set to true, the process will only display the actions that would be performed
    /// without actually renaming, moving, or deleting any files.
    #[arg(long, default_value_t = false)]
    dry_run: bool,

    /// Specifies the action option:
    /// 0 = Copy files only.
    /// 1 = Moves the files, keep directory.
    /// 2 = Moves the files and deletes the directory.
    #[arg(long, default_value_t = 0)]
    action: u8,

    /// The name of the metadata file to look for in each directory.
    /// Defaults to 'metadata.json'.
    #[arg(long, default_value_t = String::from("metadata.json"))]
    metafile: String,

    /// A comma-separated list of audio file extensions to process.
    /// Defaults to common audiobook formats.
    #[arg(long, default_value_t = String::from("m4b,m4a,m4p,mp3,aa,aax,aac,ogg,wma,wav,flac,alac"))]
    file_types: String,
}

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
struct Metadata {
    title: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    subtitle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    series: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    book_number: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    book_number_with_zeros: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    author: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    published_year: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    published_date: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    genre: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    abridged: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_number: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_number_with_zeros: Option<String>,
}

/// Represents the possible actions that can be performed on audiobook files.
///
/// This enum defines the options for copying, moving, or deleting files.
#[derive(Debug, Clone, PartialEq)]
enum ActionOpt {
    None = 0,
    Move = 1,
    All = 2,
}

#[derive(Debug)]
struct Plan {
    from: String,
    to: String,
    metadata: Metadata,
    action: ActionOpt,
}

#[derive(Debug)]
struct Config {
    from: String,
    to: String,
    action: ActionOpt,
    dry_run: bool,
    file_ext: Vec<String>,
    metafile: String,
}

/// Represents the schema used for formatting file paths and names.
///
/// This struct contains templates for generating directory paths and file names
/// based on metadata.
#[derive(Debug)]
struct Schema {
    path_template: String,
    file_template: String,
}

impl Schema {
    fn new(path: String, file: String) -> Self {
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
    fn fmt_path(&self, metadata: &mut Metadata) -> Result<String, RenderError> {
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
    fn fmt_file(
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

fn main() {
    let args = Args::parse();
    let action = match args.action {
        0 => ActionOpt::None,
        1 => ActionOpt::Move,
        2 => ActionOpt::All,
        _ => {
            println!("Unknow delete option value of '{}' set!", args.action);
            println!("Select one of the following options:");
            println!("0 = Copy files only.");
            println!("1 = Moves the files, keep directory.");
            println!("2 = Moves the files and deletes the directory.");
            exit(1)
        }
    };

    let mut file_types: Vec<String> = args
        .file_types
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();

    if file_types.is_empty() {
        file_types = [
            "m4b", "m4a", "m4p", "mp3", "aa", "aax", "aac", "ogg", "wma", "wav", "flac", "alac",
        ]
        .iter()
        .map(|&s| s.to_string())
        .collect();
    }

    if let Err(_) = CONFIG.set(Config {
        from: args.source,
        to: args.destination,
        action,
        dry_run: args.dry_run,
        file_ext: file_types,
        metafile: args.metafile,
    }) {
        eprintln!(
            "{}",
            "Error: Tried to set global config and it failed!".red()
        );
    }

    let cfg = CONFIG.get().expect("CONFIG was not set");

    if cfg.dry_run {
        println!(
            "{}",
            "--->>> This is running as a dry-run, no changes will be made! <<<---"
                .bold()
                .underline()
                .yellow()
        );
    }

    let schema = Schema::new(args.path_schema, args.file_schema);

    // Define the move/rename schema
    let plan_list = plan(&schema);
    if cfg.dry_run {
        // Dry run or execute the move/rename plan
        dry_run(&schema, plan_list);
    } else {
        run(&schema, plan_list);
    }

    println!("\n——————————————————————————————");
    println!("{}", "Finished!".bold().blue());
}

/**
 * Generate a move/rename plan for the given path and schema.
 *
 * This function takes a path and a schema as input and returns a vector of plans.
 * Each plan represents a move or rename operation that needs to be performed.
 *
 * @param schema - The schema to use for formatting the new file names.
 * @return Vec<Plan> - A vector of plans representing the move/rename operations.
 */
fn plan(schema: &Schema) -> Vec<Plan> {
    let cfg = CONFIG.get().expect("CONFIG was not set");
    let target_file = &cfg.metafile;

    println!(
        "Searching for '{}' in '{}' and all sub-directories...",
        target_file.green(),
        cfg.from.green()
    );

    let mut actions = Vec::new();
    for entry in WalkDir::new(&cfg.from) {
        match entry {
            Ok(entry) => {
                if entry.file_name().to_str() == Some(target_file.as_str()) {
                    let metadata_file = entry.path().display().to_string();
                    // read the metadata_file
                    match parse_metadata(&metadata_file) {
                        Some(mut metadata) => match schema.fmt_path(&mut metadata) {
                            Ok(value) => actions.push(Plan {
                                from: entry.path().parent().unwrap().display().to_string(),
                                to: format!("{}/{}", cfg.to, value),
                                metadata,
                                action: cfg.action.clone(),
                            }),
                            Err(_) => {
                                eprintln!(
                                    "{} '{}' - Schema: {}",
                                    "Error: Required field missing in file".red(),
                                    metadata_file.yellow(),
                                    schema.path_template.yellow()
                                );
                            }
                        },
                        None => {}
                    }
                }
            }
            Err(err) => {
                eprintln!("{}{}", "Error: ".red(), err);
            }
        }
    }

    actions
}

/**
 * Run the migration process.
 *
 * This function takes a schema and a vector of plans, and executes the migration process.
 * It creates the necessary directories and copies the files according to the provided schema.
 */
fn run(schema: &Schema, actions: Vec<Plan>) {
    let cfg = CONFIG.get().expect("CONFIG was not set");

    for mut action in actions {
        println!("--\n");
        let dde = fs::exists(&action.to);
        if !dde.unwrap_or(false) {
            match fs::create_dir_all(&action.to) {
                Ok(_) => println!("{} {}", "Created Directory:".green(), action.to),
                Err(err) => eprintln!("{} {}", "Error creating directory:".red(), err),
            }
        }

        let files: Vec<PathBuf> = get_files(&action.from);
        for file in files {
            let file_name = schema
                .fmt_file(&mut action.metadata, &file, &cfg.file_ext)
                .unwrap();
            let destination_path = format!("{}/{}", action.to, file_name);

            if action.action == ActionOpt::All || action.action == ActionOpt::Move {
                move_file(&file, &destination_path);
            } else {
                copy_file(&file, &destination_path);
            }
        }

        if action.action == ActionOpt::All {
            match fs::remove_dir_all(&action.from) {
                Ok(_) => println!("{} {}", "Deleted:".yellow(), action.from),
                Err(err) => eprintln!("{} {}", "Error deleting old directory:".red(), err),
            }

            let path = Path::new(&action.from);
            match path.parent() {
                Some(p) => {
                    for to_remove in [".DS_Store"] {
                        // Remove junk files before atempting to delete the directory
                        fs::remove_file(p.join(to_remove)).unwrap_or(());
                    }

                    match fs::remove_dir(p) {
                        Ok(_) => println!("{} '{:?}'", "Deleted:".yellow(), p),
                        Err(_) => {
                            eprintln!("{} {:?}", "Unempty directory, not deleting:".yellow(), p);
                        }
                    }
                }
                None => (),
            }
        }
    }
}

/**
 * Copy a file from one location to another.
 *
 * @param file The path of the file to copy.
 * @param destination_path The path to copy the file to.
 */
fn copy_file(file: &PathBuf, destination_path: &String) {
    print!(
        "\n{} '{}' to '{}'...",
        "Copying:".blue(),
        file.to_str().unwrap(),
        destination_path.green()
    );
    match fs::copy(&file, &destination_path) {
        Ok(_) => {
            print!(" Done\n");
        }
        Err(err) => eprintln!("{} {}", "Error copying file:".red(), err),
    }
}

/**
 * Move a file from one location to another.
 *
 * @param file The path of the file to move.
 * @param destination_path The path to move the file to.
 */
fn move_file(file: &PathBuf, destination_path: &String) {
    print!(
        "{} '{}' to '{}'...",
        "Moving:".blue(),
        file.to_str().unwrap(),
        destination_path.green()
    );
    match fs::rename(&file, &destination_path) {
        Ok(_) => {
            println!(" Done");
        }
        Err(err) => eprintln!("{} {}", "Error copying file:".red(), err),
    }
}

/**
 * Simulates the actions that would be performed during the process.
 *
 * This function prints the planned operations (e.g., file moves, deletions) without executing them.
 *
 * @param schema The schema used for formatting file paths and names.
 * @param actions A vector of `Plan` objects representing the operations to simulate.
 */
fn dry_run(schema: &Schema, actions: Vec<Plan>) {
    let cfg = CONFIG.get().expect("CONFIG was not set");

    for mut action in actions {
        println!("--\n");
        let dde = fs::exists(&action.to);
        if !dde.unwrap_or(false) {
            println!("{} {}", "Created Directory:".green(), action.to);
        }

        let files: Vec<PathBuf> = get_files(&action.from);
        for file in files {
            let file_name = schema
                .fmt_file(&mut action.metadata, &file, &cfg.file_ext)
                .unwrap();
            let destination_path = format!("{}/{}", action.to, file_name);

            if action.action == ActionOpt::Move || action.action == ActionOpt::All {
                print!(
                    "{} '{}' to '{}'...",
                    "Moving:".blue(),
                    file.to_str().unwrap(),
                    destination_path.green()
                );
            } else {
                print!(
                    "{} '{}' to '{}'...",
                    "Copying:".blue(),
                    file.to_str().unwrap(),
                    destination_path.green()
                );
            }

            println!(" Done");
        }

        if action.action == ActionOpt::All {
            println!("{} {:?}", "Deleted:".yellow(), action.from);
        }
    }
}

/**
 * Retrieves a list of audio files from the specified directory.
 *
 * @param dir The directory to search for files.
 * @return A vector of `PathBuf` objects representing the audio files found.
 */
fn get_files(dir: &String) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let dir = Path::new(dir);

    for file in WalkDir::new(dir) {
        let file = file.unwrap();
        let path = file.path();

        if path.is_file() {
            files.push(path.to_path_buf());
        }
    }

    files
}

/**
 * Parses metadata from a JSON file and converts it into a `Metadata` object.
 *
 * @param path The file path to the JSON metadata file.
 * @return An `Option` containing the parsed `Metadata` object, or `None` if parsing fails.
 */
fn parse_metadata(path: &str) -> Option<Metadata> {
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

/**
 * Get the track number from a file's metadata.
 *
 * This function attempts to extract the track number from the file's metadata.
 * If the track number is not found or is invalid, it returns None.
 */
fn get_track_number(path: &str) -> Option<u16> {
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
