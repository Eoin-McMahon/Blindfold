use colored::*;
use prettytable::{Cell, Row, Table};
use reqwest;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs::File;
use std::io::*;
use std::path::{Path, PathBuf};
use itertools::Itertools;
use strsim::normalized_levenshtein;

static GLOBSTAR: &str = "**";

#[derive(Serialize, Deserialize, Debug)]
pub struct FileRes {
    name: String,
    download_url: Option<String>,
}

// performs a http GET request using the reqwest crate
pub fn http_get(url: &str) -> String {
    let response = reqwest::get(url)
        .expect("Error: Url Not Found")
        .text()
        .expect("Error: Text unextractable from url");

    return response;
}

// builds a mapping of template names to urls to download them
pub fn build_file_map(res: &str) -> HashMap<String, String> {
    // parse json response to extract name and download link into FileRes struct
    let all_files: Vec<FileRes> = serde_json::from_str(res).unwrap();

    // filter out non-gitignore files
    let gitignore_files: Vec<&FileRes> = all_files
        .iter()
        .filter(|file| file.name.contains("gitignore"))
        .collect();

    // destructure vec of structs to vec of tuples in form (name, url)
    let destructured: Vec<(String, String)> = gitignore_files
        .iter()
        .map(|file| destructure_to_tup(file))
        .collect();

    // collect vector of tuples into a hashmap
    let file_map: HashMap<String, String> = destructured.into_iter().collect();

    return file_map;
}

// destructure FileRes struct to a tuple of its fields
pub fn destructure_to_tup(file_struct: &FileRes) -> (String, String) {
    // format name to be language name lowercased
    let name: String = file_struct
        .name
        .clone()
        .replace(".gitignore", "")
        .to_lowercase();

    let mut url: String = String::from("");

    if let Some(download_url) = &file_struct.download_url {
        url.push_str(download_url);
    }

    return (name, url);
}

// make http get request for the specified template and return the raw text of the gitignore as a string
pub fn get_raw_ignore_file(file_map: &HashMap<String, String>, lang: &str) -> String {
    let mut response: String = String::from("");
    let file_url: Option<&String> = file_map.get(lang);

    if let Some(file) = file_url {
        response.push_str(&http_get(&file));
    }

    return response;
}

// Coalesces consecutives globstars into a single globstar
pub fn reduce_globstars(path: &str) -> String {
    let path_parts = path.split("/");
    let coalesced_parts = path_parts.coalesce(|x, y| {
        if x == GLOBSTAR && y == GLOBSTAR {
            Ok(GLOBSTAR)
        } else {
            Err((x, y))
        }
    });

     return coalesced_parts.collect::<Vec<&str>>().join("/");
}

// Add title for each raw gitignore and add prefix paths to all non-comment lines
fn format_gitignore(raw_body: &String, prefix_path: Option<&Path>, language: &str) -> String {
    let mut body = String::with_capacity(raw_body.len() * 2);

    if let Some(path) = prefix_path {
        for line in raw_body.lines() {
            // Check if the line is a comment or empty by consuming all the whitespace and then checking
            // if the next character is a hash
            let first_non_whitespace_char =
                line.chars().skip_while(|c| c.is_ascii_whitespace()).next();

            if first_non_whitespace_char == Some('#') || first_non_whitespace_char == None {
                // If the line is a comment or blank then add it to the file untouched
                body.push_str(line);
            } else {
                // Trim the '!' off the input line (if it exists) and add it to the start of the
                // output line
                let trimmed_line = if first_non_whitespace_char == Some('!') {
                    // The line is an exclusion, so remove the '!' from the start of the path and
                    // add it to the output string
                    body.push('!');
                    &line[1..]
                } else {
                    line
                };

                // A lot of gitignores seem to have erroneous '/'s at the start of their paths, but
                // rust is not magic so it can't figure out which ones are actually correct so this
                // will just remove them all
                let corrected_line = if trimmed_line.chars().next() == Some('/') {
                    trimmed_line.chars().skip(1).collect::<String>()
                } else {
                    trimmed_line.to_string()
                };

                let prefixed_path = path.join(Path::new(&corrected_line));
                let prefixed_path_str = prefixed_path.to_str().expect("Path is not valid unicode");
                let final_path_str = &reduce_globstars(prefixed_path_str);
                let final_path = Path::new(final_path_str);


                body.push_str(
                    final_path
                        .to_str()
                        .expect("Bad path found in gitignore."),
                );
            }

            body.push('\n');
        }

        // Remove the newline at the end of the body
        assert_eq!(body.pop(), Some('\n'));
    } else {
        // The prefix path is None, so we can just copy the body as-is
        body.push_str(raw_body);
    }

    let ignore_template: String = format!(
        "# {} gitignore generated by Blindfold\n\n{}\n\n",
        language.to_uppercase(),
        body
    );

    println!("Generated .gitignore for {} 🔧", language.magenta().bold());
    return ignore_template;
}

// returns formatted gitignore string for each language provided
pub fn generate_gitignore_file(languages: Vec<&str>, file_map: &HashMap<String, String>) -> String {
    // string to store all the gitignores
    let mut gitignore: String = String::from("");

    // generate gitignore for each language and append to output string
    for path_and_language in languages.iter() {
        // Split the path and language, with path being None if the language name doesn't contain a
        // '/'
        let last_slash_index = path_and_language.rfind('/');
        let language = &path_and_language[last_slash_index.map_or(0, |x| x + 1)..];
        let prefix_path = last_slash_index.map(|i| Path::new(&path_and_language[..i + 1]));

        // make sure a language is added
        if language == "" {
            continue;
        }

        if file_map.contains_key(&language.to_string()) {
            let ignore_body: String = get_raw_ignore_file(&file_map, language);
            gitignore.push_str(&format_gitignore(&ignore_body, prefix_path, language));
        } else {
            let stdio = stdin();
            let input = stdio.lock();
            let output = stdout();

            let most_similar: Option<String> =
                suggest_most_similar(input, output, language.clone(), file_map.clone());

            if let Some(language) = most_similar {
                let ignore_body: String = get_raw_ignore_file(&file_map, &language);
                gitignore.push_str(&format_gitignore(&ignore_body, prefix_path, &language));
            }
        }
    }

    return gitignore;
}

// given a mis-typed language this function returns the most similar language available
pub fn suggest_most_similar<R, W>(
    mut reader: R,
    mut writer: W,
    typo: &str,
    file_map: HashMap<String, String>,
) -> Option<String>
where
    R: BufRead,
    W: Write,
{
    // find language most similar to what was requested
    let mut max: f64 = 0.0;
    let mut most_similar: String = String::new();

    for candidate in file_map.keys() {
        let similarity: f64 = normalized_levenshtein(typo, candidate);
        if similarity > max {
            most_similar = candidate.to_string();
            max = similarity;
        }
    }

    // take input to accept/deny suggestion
    write!(
        &mut writer,
        "Couldn't generate template for {}, did you mean {}? [y/N]: ",
        typo.yellow().bold(),
        most_similar.bright_green().bold()
    )
    .expect("Unable to write");
    // flush input buffer so that it prints immediately
    stdout().flush().ok();

    let mut choice: String = String::new();
    reader
        .read_line(&mut choice)
        .ok()
        .expect("Couldn't read line");

    if choice.to_lowercase().trim() == String::from("y") {
        return Some(most_similar);
    }

    return None;
}

// writes gitignore string to file
pub fn write_to_file(dest: &str, gitignore: String) -> std::io::Result<()> {
    let filepath: PathBuf = Path::new(dest).join(".gitignore");
    println!(
        "Writing file to {}... ✏️ ",
        filepath
            .to_str()
            .expect("Unknown output file name.")
            .bright_blue()
            .bold()
    );
    let mut file = File::create(filepath)?;
    file.write_all(gitignore.as_bytes())?;
    println!("{} ✨", "Done!".green().bold());

    Ok(())
}

// add gitignore to existing gitignore file
pub fn append_to_file(destination: &str, gitignore: String) -> std::io::Result<()> {
    let filepath: PathBuf = Path::new(destination).join(".gitignore");

    // open existing gitignore and concatenate with new template
    let mut file = File::open(&filepath)?;
    let mut existing: String = String::new();
    file.read_to_string(&mut existing)?;
    let combined: String = format!("{}{}", existing, gitignore);

    if !combined.is_empty() {
        println!(
            "Loaded existing gitignore file from {} 💾",
            filepath
                .to_str()
                .expect("Unknown file path.")
                .bright_blue()
                .bold()
        );

        // write it to file
        write_to_file(destination, combined).expect("Couldn't write to file ⚠️ ");
    }

    return Ok(());
}

// print a table containing all available templates for generation
pub fn list_templates(file_map: HashMap<String, String>) {
    let mut table = Table::new();

    let mut keys: Vec<String> = file_map.keys().map(|key| key.clone()).collect();

    keys.sort();

    let mut chunks = keys.chunks(4);

    // while another row can be constructed, construct one and add to table
    while let Some(chunk) = chunks.next() {
        // map chunk items to cell
        let cells = chunk.iter().map(|item| Cell::new(item)).collect();

        let row = Row::new(cells);
        table.add_row(row);
    }

    // print table
    table.printstd();
}
