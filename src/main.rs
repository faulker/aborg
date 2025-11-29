mod metadata;
mod schema;
mod track;

use clap::Parser;
use colored::Colorize;
use metadata::{Metadata, parse_metadata};
use schema::Schema;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::exit;
use std::sync::OnceLock;
use walkdir::WalkDir;

// TODO:
// - Add a "results" output at the end that prints total files touched, etc, also have it output a list of any errors
// - Fix the bug where the source dir is not being deleted when empty
// - Add chapter filtering from file name

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
