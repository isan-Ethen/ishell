use ishell::*;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::{env, fs, io, io::Write, path::Path, thread};

fn main() {
    let home_dir = env::var("HOME").unwrap();
    let home_path = Path::new(&home_dir);
    let home_str = &home_dir;

    loop {
        let mut buffer = String::new();
        let current_dir = env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap();
        // let current_dir = env::var("PWD").unwrap();
        loop {
            print!("{}$ ", current_dir.replace(home_str, "~"));
            io::stdout().flush().unwrap();

            match io::stdin().read_line(&mut buffer) {
                Ok(_) => break,
                Err(e) => {
                    eprintln!("Can not read line: {e}");
                }
            };
        }
        let channel = Channel::new();
        thread::scope(|s| {
            s.spawn(|| {
                let mut inputs = buffer.split_whitespace();
                let response = match inputs.next() {
                    Some("ls") => {
                        let files = if let Some(directory) = inputs.next() {
                            fs::read_dir(directory).unwrap()
                        } else {
                            fs::read_dir(current_dir.clone()).unwrap()
                        };
                        for file in files.into_iter() {
                            let filename = file.unwrap().file_name().into_string().unwrap();
                            let file_vec = filename.split("/");
                            println!("{}", file_vec.last().unwrap());
                        }
                        println!("");
                        Response::Success
                    }
                    Some("echo") => {
                        inputs.into_iter().for_each(|arg| print!("{} ", arg));
                        println!("");
                        Response::Success
                    }
                    Some("cd") => {
                        if let Some(path) = inputs.next() {
                            Response::Redirection(Command::Cd(Box::new(std::path::Path::new(path))))
                        } else {
                            Response::Redirection(Command::Cd(Box::new(home_path)))
                        }
                    }
                    Some("pwd") => {
                        println!("{}", current_dir);
                        Response::Success
                    }
                    Some("clear") => {
                        println!("\x1B[2J\x1B[1;1H");
                        Response::Success
                    }
                    Some("cat") => {
                        for filename in inputs.into_iter() {
                            match File::open(&filename) {
                                Err(why) => eprintln!("Failed to open {}: {}", filename, why),
                                Ok(buffer) => {
                                    let contents = BufReader::new(buffer);
                                    for line in contents.lines() {
                                        println!("{}", line.unwrap());
                                    }
                                }
                            }
                        }
                        Response::Success
                    }
                    Some("exit") => Response::Exit,
                    Some(not_found) => Response::Error(format!("Command {} not found", not_found)),
                    None => Response::Continue,
                };
                channel.send(response);
            });
        });

        let response = channel.receive();
        match response {
            Response::Success => {}
            Response::Redirection(command) => match command {
                // Command::Cd(path) => env::set_var("PWD", path),
                Command::Cd(path) => match env::set_current_dir(*path) {
                    Ok(_) => {}
                    Err(why) => eprintln!("Couldn't change directory: {}", why),
                },
            },
            Response::Error(error) => eprintln!("Error occured: {}", error),
            Response::Continue => {}
            Response::Exit => {
                println!("Thank you for using ishell!!");
                break;
            }
        }
        println!("");
    }
}
