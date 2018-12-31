#[macro_use]
extern crate structopt;
extern crate confy;
#[macro_use]
extern crate serde_derive;
extern crate serde;
extern crate serde_json;
use structopt::StructOpt;
extern crate zip;
#[macro_use]
extern crate human_panic;
extern crate gog;
extern crate indicatif;
mod args;
mod config;
use crate::args::Connect::*;
use crate::args::Wyvern;
use crate::args::Wyvern::Download;
use crate::args::Wyvern::*;
use crate::config::Config;
use gog::extract::*;
use gog::gog::{connect::*, connect::ConnectGameStatus::*, FilterParam::*, *};
use gog::token::Token;
use gog::Error;
use gog::Gog;
use indicatif::{ProgressBar, ProgressStyle};
use std::fs;
use std::fs::OpenOptions;
use std::fs::*;
use std::io;
use std::io::Read;
use std::io::Write;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
fn main() -> Result<(), ::std::io::Error> {
    #[cfg(not(debug_assertions))]
    setup_panic!();
    let mut config: Config = confy::load("wyvern")?;
    if config.token.is_none() {
        let token = login();
        config.token = Some(token);
    }
    config.token = Some(config.token.unwrap().refresh().unwrap());
    print!("");
    let gog = Gog::new(config.token.clone().unwrap());
    confy::store("wyvern", config)?;
    let args = Wyvern::from_args();
    match args {
        List { id } => {
            if let Some(id) = id {
                let details = gog.get_game_details(id).unwrap();
                println!("Title - GameID");
                println!("{} - {}", details.title, id);
            } else {
                list_owned(gog).unwrap();
            }
        }
        Download { id, search , install_after} => {
            if let Some(search) = search {
                let search_results =
                    gog.get_filtered_products(FilterParams::from_one(Search(search)));
                if search_results.is_ok() {
                    let e = search_results.unwrap();
                    for (idx, pd) in e.iter().enumerate() {
                        println!("{}. {} - {}", idx, pd.title, pd.id);
                    }
                    let mut choice = String::new();
                    loop {
                        print!("Select a game to download:");
                        io::stdout().flush().unwrap();
                        io::stdin().read_line(&mut choice).unwrap();
                        let parsed = choice.trim().parse::<usize>();
                        if let Ok(i) = parsed {
                            if e.len() > i {
                                let details = gog.get_game_details(e[i].id).unwrap();
                                let name = download_prep(gog, details).unwrap();
                    if install_after.is_some() {
                        println!("Installing game");
                        let mut installer = fs::File::open(name).unwrap();
                        install(&mut installer, install_after.unwrap());
                     }
                                break;
                            } else {
                                println!("Please enter a valid number corresponding to an available download");
                            }
                        } else {
                            println!(
                                "Please enter a number corresponding to an available download"
                            );
                        }
                    }
                } else {
                    println!("Could not find any games.");
                }
            } else if let Some(id) = id {
                let details = gog.get_game_details(id).unwrap();
                let name = download_prep(gog, details).unwrap();
                  if install_after.is_some() {
                        println!("Installing game");
                        let mut installer = fs::File::open(name).unwrap();
                        install(&mut installer, install_after.unwrap());
                     }

            } else {
                println!("Did not specify a game to download");
            }
        }
        Install {
            installer_name,
            path,
        } => {
            let mut installer = File::open(&installer_name);
            if installer.is_ok() {
                install(&mut installer.unwrap(), path);
            } else {
                println!("File {} does not exist", installer_name)
            }
        },
        Connect { .. } => {
            let uid: i64 = gog.get_user_data().unwrap().user_id.parse().unwrap();
            let linked = gog.connect_account(uid);
            if linked.is_err() {
                println!("You don't have a steam account linked to GOG! Go to https://www.gog.com/connect to link one.");
                return Ok(());
            } else {
                gog.connect_scan(uid).unwrap();
            }
            match args {
                Connect(ListConnect { claim, quiet }) => {
                    let mut items = gog.connect_status(uid).unwrap().items;
                    let left_over: Vec<(String, ConnectGame)> = items
                        .into_iter()
                        .filter_map(|x| {
                            if !claim || x.1.status == READY_TO_LINK {
                                let details = gog.product(vec![x.1.id], vec![]);
                                if details.is_ok() {
                                    println!("{} - {:?}", details.unwrap()[0].title, x.1.status);
                                    return None;
                                }
                            }
                            return Some(x);
                        })
                        .collect();
                    if !quiet {
                        println!("{} items not shown due to options", left_over.len());
                    }
                }
                Connect(ClaimAll {}) => {
                    gog.connect_claim(uid).unwrap();
                    println!("Claimed all available games");
                }
                _ => println!("Tell someone about this, because it should not be happening"),
            }
        }
    };
    Ok(())
}
fn download_prep(gog: Gog, details: GameDetails) -> Result<String, Error> {
                if details.downloads.linux.is_some() {
                let name = download(gog, details.downloads.linux.unwrap()).unwrap();
                return Ok(name);
                  } else {
                    let mut choice = String::new();
                    loop {
                        println!("This game does not support linux! Would you like to download the windows version to run under wine?(y/n)");
                        io::stdout().flush().unwrap();
                        io::stdin().read_line(&mut choice).unwrap();
                        match choice.to_lowercase().as_str() {
                            "y" =>  {
                                println!("Downloading windows files. Note: wyvern does not support automatic installation from windows games");
                                let name = download(gog, details.downloads.windows.unwrap()).unwrap();
                                return Ok(name);
                            },
                            "n" => {
                                println!("No suitable downloads found. Exiting");
                                std::process::exit(0);
                            },
                            _ => println!("Please enter y or n to proceed."),
                        }
                    }
                    
                }

}
fn install (installer: &mut File, path: PathBuf) {
                extract(
                    installer,
                    "/tmp",
                    ToExtract {
                        unpacker: false,
                        mojosetup: false,
                        data: true,
                    },
                )
                .unwrap();
                let mut file = File::open("/tmp/data.zip").unwrap();
                // Extract code taken mostly from zip example
                let mut archive = zip::ZipArchive::new(file).unwrap();
                for i in 0..archive.len() {
                    let mut file = archive.by_index(i).unwrap();
                    let filtered_path = file.sanitized_name().to_str().unwrap().replace("/noarch", "").replace("data/","").to_owned();
                    //Extract only files for the game itself
                    if filtered_path.contains("game") {
                    let outpath = path.join(PathBuf::from(filtered_path));

                    if (&*file.name()).ends_with('/') {
                        println!(
                            "File {} extracted to \"{}\"",
                            i,
                            outpath.as_path().display()
                        );
                        fs::create_dir_all(&outpath).unwrap();
                    } else {
                        if let Some(p) = outpath.parent() {
                            if !p.exists() {
                                fs::create_dir_all(&p).unwrap();
                            }
                        }
                        println!("{:?}", outpath);
                        let mut outfile = fs::File::create(&outpath).unwrap();
                        io::copy(&mut file, &mut outfile).unwrap();
                    }
                    use std::os::unix::fs::PermissionsExt;
                    if let Some(mode) = file.unix_mode() {
                        fs::set_permissions(&outpath, fs::Permissions::from_mode(mode)).unwrap();
                    }
                }
                }
}
pub fn login() -> Token {
    println!("It appears that you have not logged into GOG. Please go to the following URL, log into GOG, and paste the code from the resulting url's ?code parameter into the input here.");
    println!("https://login.gog.com/auth?client_id=46899977096215655&layout=client2%22&redirect_uri=https%3A%2F%2Fembed.gog.com%2Fon_login_success%3Forigin%3Dclient&response_type=code");
    io::stdout().flush().unwrap();
    let mut code = String::new();
    let mut token: Token;
    loop {
        io::stdin().read_line(&mut code).unwrap();
        let attempt_token = Token::from_login_code(code.as_str());
        if attempt_token.is_ok() {
            token = attempt_token.unwrap();
            println!("Got token. Thanks!");
            break;
        } else {
            println!("Invalid code. Try again!");
        }
    }
    token
}
fn list_owned(gog: Gog) -> Result<(), Error> {
    let games = gog.get_filtered_products(FilterParams::from_one(MediaType(1)))?;
    println!("Title - GameID");
    for game in games {
        println!("{} - {}", game.title, game.id);
    }
    Ok(())
}
fn download(gog: Gog, downloads: Vec<gog::gog::Download>) -> Result<String, Error> {
        let mut names = vec![];
        for download in downloads.iter() {
            names.push(download.name.clone());
        }
        let mut responses = gog.download_game(downloads);
        let count = responses.len();
        for (idx, mut response) in responses.iter_mut().enumerate() {
            let total_size = response
                .headers()
                .get("Content-Length")
                .unwrap()
                .to_str()
                .unwrap()
                .parse()
                .unwrap();
            let pb = ProgressBar::new(total_size);
            pb.set_style(ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {bytes}/{total_bytes} ({eta})")
            .progress_chars("#>-"));
            let name = names[idx].clone();
            println!("Downloading {}, {} of {}", name, idx + 1, count);
            let mut fd = fs::File::create(name.clone())?;
            let mut perms = fd.metadata()?.permissions();
            perms.set_mode(0o744);
            fd.set_permissions(perms)?;
            let mut pb_read = pb.wrap_read(response);
            io::copy(&mut pb_read, &mut fd)?;
            pb.finish();
        }
        println!("Done downloading!");
        return Ok(names[0].clone());
}
