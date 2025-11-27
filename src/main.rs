use clap::Parser;
use colored::Colorize;
use handlebars::{Handlebars, RenderError, no_escape};
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
            let file_number = parse_file_number(&file_name);
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

            match cfg.action {
                ActionOpt::Move | ActionOpt::All => println!("{} {:?}", "Deleted:".yellow(), file),
                _ => {}
            }
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
 * Extracts the file number from a file name.
 *
 * The file number is typically a whole number found at the start of the file name.
 * If multiple numbers are present, context-based rules are applied to determine the correct one.
 *
 * @param file_name The name of the file to analyze.
 * @return An `Option<u16>` containing the extracted file number, or `None` if no valid number is found.
 */
fn parse_file_number(file_name: &str) -> Option<u16> {
    // 1. Priority 1: Leading Number (e.g., "02 - ...")
    // If the file starts with a number followed by a hyphen, this is the file number.
    let re_start = Regex::new(r"^\s*(\d+)\s*-").unwrap();
    if let Some(caps) = re_start.captures(file_name) {
        return caps[1].parse().ok();
    }

    // 2. Priority 2: Context Keywords (Section, Chapter, Part)
    // We also look for "Book" here to define what number to IGNORE.
    let re_context = Regex::new(r"(?i)\b(section|chapter|part|book)\s*#?\s*(\d+)\b").unwrap();

    let mut ignore_num: Option<&str> = None;

    for cap in re_context.captures_iter(file_name) {
        let keyword = cap.get(1).unwrap().as_str();
        let num_str = cap.get(2).unwrap().as_str();

        if keyword.eq_ignore_ascii_case("book") {
            // If we see "Book 3", we store "3" as a number to ignore later.
            ignore_num = Some(num_str);
        } else {
            // If we see Chapter/Part/Section, we trust this is the file number
            // and return it immediately.
            return num_str.parse().ok();
        }
    }

    // 3. Fallback: Find the first number that isn't the "Book" number.
    // This handles cases like "Track 01" or "2025 Title" or simple "03".
    let re_digits = Regex::new(r"\d+").unwrap();

    for mat in re_digits.find_iter(file_name) {
        let num_str = mat.as_str();

        // If this number matches the "Book" number we found earlier, skip it.
        if let Some(ignored) = ignore_num {
            if num_str == ignored {
                continue;
            }
        }

        // Otherwise, return the first valid number we find.
        return num_str.parse().ok();
    }

    // If we get here, no valid numbers were found (or the only number was a Book number).
    None
}
