use std::path::{Path, PathBuf};
use std::fs::File;
use std::io::prelude::*;
use clap::{Arg, App};
mod lib;
use std::collections::HashMap;

// api end point for the gitignore templates repository
const API_URL: &str = "https://api.github.com/repos/toptal/gitignore/contents/templates?ref=master";

fn main() -> std::io::Result<()> {
    let matches = App::new("IgnoriGen")
                    .version("1.0")
                    .author("Eoin McMahon <eoin.mcmahon.dev@gmail.com>")
                    .about("Grabs gitignore templates from gitignore.io")
                    .arg(Arg::with_name("LANGUAGE(S)")
                         .short("l")
                         .long("lang")
                         .takes_value(true)
                         .multiple(true)
                         .required(true)
                         .help("template(s) to generate gitignore for i.e Rust, Flutter, VsCode etc."))
                    .arg(Arg::with_name("DESTINATION")
                        .short("d")
                        .long("dest")
                        .help("Destination to store the gitignore file in")
                        .takes_value(true))
                    .get_matches();

    let languages: Vec<&str>= matches.values_of("LANGUAGE(S)").unwrap().collect();
    let destination: &str = matches.value_of("DESTINATION").unwrap_or("./");

    println!("Generating .gitignore files for: {:?}", languages);
    println!("Storing .gitignore in: {:?}", destination);

    // perform a get request to list the gitignore repository files
    let repo_contents: String = lib::http_get(API_URL);
    let file_map: HashMap<String, String> = lib::build_file_map(&repo_contents);

    let mut gitignore_template: String = String::new();

    for language in languages.iter() {
        // get gitignore template for each language and push to one 'mega-gitignore'
        let ignore_body: String = lib::get_ignore_file(&file_map, language);
        let ignore_template: String = format!("{} gitignore \n\n {} \n\n", language.to_uppercase(), ignore_body);
        gitignore_template.push_str(&ignore_template);
    }

    // path to the gitignore file to be generated
    let filepath: PathBuf = Path::new(destination).join(".gitignore");
    
    // create and write to file
    let mut file = File::create(filepath)?;
    file.write_all(gitignore_template.as_bytes())?;

    println!(".gitignore file generated!!");
    
    return Ok(());
}
