mod command;
mod line_parser;

use command::Command;

use nix::unistd::close;
use std::os::fd::RawFd;

use std::{env, io, io::Write};

pub struct Shell {
    history: Vec<String>,
    home_directory: String,
    current_directory: String,
}

impl Shell {
    fn new() -> Shell {
        Shell {
            history: Vec::new(),
            home_directory: env::var("HOME").expect("Couldn't get HOME"),
            current_directory: Shell::get_current_directory(),
        }
    }

    pub fn run() {
        let mut shell = Shell::new();

        loop {
            shell.set_current_directory();
            let buffer = shell.read_line();
            shell.history.push(buffer.clone());
            let (mut commands, mut pipev) = shell.parse(buffer);
            println!("{:?}", commands);
            Shell::commands_execute(&mut commands, &mut pipev);
        }
    }

    fn set_current_directory(&mut self) {
        self.current_directory = Shell::get_current_directory();
    }

    fn get_current_directory() -> String {
        env::current_dir()
            .unwrap()
            .into_os_string()
            .into_string()
            .unwrap()
    }

    fn read_line(&self) -> String {
        let mut buffer = String::new();
        loop {
            self.print_prompt();
            io::stdout().flush().unwrap();

            match io::stdin().read_line(&mut buffer) {
                Ok(_) => return buffer,
                Err(why) => {
                    eprintln!("Can not read line: {}", why);
                }
            };
        }
    }

    fn print_prompt(&self) {
        print!(
            "{}$ ",
            self.current_directory.replace(&self.home_directory, "~")
        );
    }

    fn commands_execute(commands: &mut Vec<Command>, pipev: &mut Vec<(RawFd, RawFd)>) {
        let mut commands = commands.iter();
        let mut pipev = pipev.iter();
        let mut is_pipe = false;

        while let Some(command) = commands.next() {
            if command.get_outfd().is_some() {
                is_pipe = true;
            }
            if command.get_infd().is_some() && is_pipe {
                // println!("{:?}", pipev);
                if let Some((_, outpipe)) = pipev.next() {
                    close(*outpipe).expect("Close outpipe failed");
                    // close(*inpipe).expect("Close inpipe failed");
                    // println!("Pipe closed");
                }
            }
            command.execute();
        }
    }
}
