# aborg (Audiobook Organizer)
**NOTE: This tool is still in development and is currently in a beta stage.**

aborg is a command-line tool that tidies up a messy audiobook collection. It renames your audiobook files and moves them into a clean, consistent folder structure like `Author/Series/Title`.

It works by reading the `metadata.json` file that [Audiobookshelf](https://www.audiobookshelf.org/) saves next to each book. That file holds the author, series, title, book number, and so on. aborg uses that information, plus the chapter or part number found in each file's name, to decide where every file goes and what it should be called.

## What it does

Say you have a folder of downloads that looks like this:

```
Downloads/
├── mistborn alloy/
│   ├── metadata.json
│   ├── alloy_of_law_part1.m4a
│   ├── alloy_of_law_part2.m4a
│   └── cover.jpg
└── Project Hail Mary (Unabridged)/
    ├── metadata.json
    └── PHM.m4b
```

After running aborg with the default settings, your collection looks like this:

```
Audiobooks/
├── Brandon Sanderson/
│   └── The Mistborn Saga/
│       └── The Alloy of Law - Book 01/
│           ├── The Mistborn Saga - The Alloy of Law (001).m4a
│           ├── The Mistborn Saga - The Alloy of Law (002).m4a
│           ├── cover.jpg
│           └── metadata.json
└── Andy Weir/
    └── Project Hail Mary/
        ├── Project Hail Mary.m4b
        └── metadata.json
```

A few things to know:

- Every folder that contains a `metadata.json` is treated as one book. Folders without one are skipped.
- Audio files are renamed. Other files (cover images, the `metadata.json` itself, etc.) are brought along with their names unchanged.
- You control the folder and file naming with templates. See [Schemas](#schemas).

---

## Install

**Download a release:** Prebuilt binaries for Linux and macOS (Apple Silicon) are on the [releases page](https://github.com/faulker/aborg/releases).

**Build from source:** You'll need [Rust](https://www.rust-lang.org/tools/install) installed.

```bash
git clone https://github.com/faulker/aborg.git
cd aborg
cargo build --release
# The binary is at target/release/aborg
```

---

## Getting started

### 1. Get a `metadata.json` for each book

aborg doesn't look up book information itself. It relies on Audiobookshelf to do that.

1. Put your unorganized audiobooks in a folder that Audiobookshelf uses as a library, and let it scan them.
2. In Audiobookshelf's settings, turn on **Store metadata with item**. This makes it save a `metadata.json` file inside each book's folder.
3. Match each book to the correct entry (for example, using Audible as the metadata provider). This fills in the author, series, title, and so on.

### 2. Preview the changes with a dry run

Always start with `--dry-run`. It shows exactly what would happen without touching any files.

```bash
aborg --source /path/to/unorganized --destination /path/to/collection --action 2 --dry-run
```

The output is grouped by book. Each book shows where it comes **From**, where it's going **To**, and then every file with its current name and new name underneath:

```
Book 1 of 2 ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
  From: /path/to/unorganized/mistborn alloy
  To:   /path/to/collection/Brandon Sanderson/The Mistborn Saga/The Alloy of Law - Book 01 (new directory)

  Move  alloy_of_law_part1.m4a
     -> The Mistborn Saga - The Alloy of Law (001).m4a
  Move  alloy_of_law_part2.m4a
     -> The Mistborn Saga - The Alloy of Law (002).m4a
```

If two files would end up with the same name in the same place (so one would overwrite the other), the dry run shows a red **WARNING** under that file, and a count of these at the end. This usually means the file names didn't contain a usable part number, or your file schema needs a number in it.

### 3. Run it for real

Once the preview looks right, run the same command without `--dry-run`:

```bash
aborg --source /path/to/unorganized --destination /path/to/collection --action 2
```

### 4. Rescan

If your destination is an Audiobookshelf library, rescan it so it picks up the moved files.

---

## Choosing an action

The `--action` option decides what happens to your original files.

| Value | What it does |
| :--- | :--- |
| `0` (default) | **Copy.** Your originals are left untouched. The safest choice. |
| `1` | **Move.** Files are moved to the destination. The now-empty source folders are left behind. |
| `2` | **Move and clean up.** Files are moved, then each book's source folder is deleted. If the folder above it is empty afterward, that is deleted too. |

---

## Usage

```bash
aborg [OPTIONS] --source <SOURCE> --destination <DESTINATION>

Options:
  -s, --source <SOURCE>            The directory containing the audiobook files you want to manage. This is the source directory for the operation
  -d, --destination <DESTINATION>  The directory where the managed files will be moved. This is the destination directory for the operation
  -p, --path-schema <PATH_SCHEMA>  The schema used to format the newly created destination directories. This uses the Handlebar schema style [default: "{{author}}/{{#if series}}{{series}}/{{/if}}{{title}}{{#if book_number_with_zeros}} - Book {{book_number_with_zeros}}{{/if}}"]
  -f, --file-schema <FILE_SCHEMA>  The schema used to format the files that are being moved. This uses the Handlebar schema style [default: "{{#if series}}{{series}} - {{/if}}{{title}}{{#if file_number_with_zeros}} ({{file_number_with_zeros}}){{/if}}"]
      --dry-run                    If set to true, the process will only display the actions that would be performed without actually renaming, moving, or deleting any files
      --action <ACTION>            Specifies the action option: [default: 0]
                                            0 = Copy files only.
                                            1 = Moves the files, keep directory.
                                            2 = Moves the files and deletes the directory
      --metafile <METAFILE>        The name of the metadata file to look for in each directory. Defaults to 'metadata.json' [default: metadata.json]
      --file-types <FILE_TYPES>    A comma-separated list of audio file extensions to process. Defaults to common audiobook formats [default: m4b,m4a,m4p,mp3,aa,aax,aac,ogg,wma,wav,flac,alac]
  -h, --help                       Print help
  -V, --version                    Print version
```

---

## Schemas

Schemas are templates that decide how folders and files are named. They use [Handlebars](https://handlebarsjs.com/guide/) syntax: a field name in double curly braces, like `{{author}}`, gets replaced with that book's value. `{{#if series}}...{{/if}}` only includes the text inside when the book has that field.

There are two schemas:

- **Path schema** (`--path-schema`) builds the folder each book goes into, inside your destination.
- **File schema** (`--file-schema`) builds the new name for each audio file. The original file extension is added automatically.

These are the defaults:

- **Path:** `{{author}}/{{#if series}}{{series}}/{{/if}}{{title}}{{#if book_number_with_zeros}} - Book {{book_number_with_zeros}}{{/if}}`
    - **Example result:** `Brandon Sanderson/The Mistborn Saga/The Alloy of Law - Book 01`

- **File:** `{{#if series}}{{series}} - {{/if}}{{title}}{{#if file_number_with_zeros}} ({{file_number_with_zeros}}){{/if}}`
    - **Example single file result:** `The Mistborn Saga - The Alloy of Law.m4b`
    - **Example multiple audio files result:**
    ```
    The Mistborn Saga - The Alloy of Law (001).m4a
    The Mistborn Saga - The Alloy of Law (002).m4a
    ...
    ```

For example, to drop the series folder and put the year in the book folder name:

```bash
aborg -s ./in -d ./out --path-schema "{{author}}/{{title}} ({{published_year}})" --dry-run
```

If the path schema uses a field that a book doesn't have (outside of an `{{#if}}`), that book is skipped and an error is shown. Wrap optional fields in `{{#if}}` in both schemas.

### Fields from `metadata.json`

| **Field** | **Description** |
| :--- | :--- |
| author | The book's author |
| series | If the book is part of a series, this will be the first entry in the `series` array |
| title | The title of the book |
| subtitle | Extra title text, book tagline, etc. |
| book_number | Book number in the series |
| book_number_with_zeros | Book number with a leading zero (e.g. `01`) |
| published_year | Year the book was published |
| published_date | Date the book was published |
| genre | The first genre in the genre array |
| language | The language the book is in |
| abridged | True if the book is abridged |

### Fields from each audio file

These come from the audio file itself: its track number tag if it has one, otherwise a number found in the file name.

| **Field** | **Description** |
| :--- | :--- |
| file_number | Number of the audio file in the book (example: 9 in "Random Book Title - Section 9.mp3") |
| file_number_with_zeros | The same as `file_number` padded to three digits (example: `009` or `016`) |

---

## Development

```bash
cargo build --release   # Build
cargo test              # Run the tests
./run.sh                # Copies ./test_bak to ./test and runs aborg on it, writing to ./output
./build.sh              # Cross-compile a static Linux (x86_64 musl) binary. Needs zig and cargo-zigbuild
```
