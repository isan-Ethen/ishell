use crate::shell::Command;
use crate::shell::Shell;

use nix::{
    fcntl::{open, OFlag},
    sys::stat::Mode,
    unistd::{chdir, pipe},
};
use std::iter::Peekable;
use std::os::fd::RawFd;
use std::path::PathBuf;
use std::{ffi::CString, fs};

impl Shell {
    pub fn parse(&self, buffer: String) -> (Vec<Command>, Vec<(RawFd, RawFd)>) {
        let mut inputs: Vec<&str> = buffer.split_whitespace().collect();
        inputs.push(";");
        let mut args: Vec<CString> = Vec::new();
        let mut commands: Vec<Command> = Vec::new();
        let mut fd: Option<RawFd> = None;
        let mut pipev: Vec<(RawFd, RawFd)> = Vec::new();

        let mut iter = inputs.iter().peekable();

        while let Some(&arg) = iter.next() {
            match arg {
                "cd" => {
                    self.handle_cd(&mut iter);
                }
                "<" => {
                    self.handle_langle(&mut args, &mut commands, &mut iter);
                }
                ">" => {
                    self.handle_rangle(&mut args, &mut commands, &mut fd, &mut iter);
                }
                "*" => {
                    self.handle_wildcard(&mut args);
                }
                "|" => {
                    Shell::handle_pipe(&mut args, &mut commands, &mut fd, &mut pipev);
                }
                ";" => {
                    Shell::handle_semicolon(&mut args, &mut commands, &mut fd);
                }
                _ => {
                    args.push(CString::new(arg).expect("Can not cast arg to CString"));
                }
            }
        }
        (commands, pipev)
    }

    fn handle_cd<'a, I>(&self, iter: &mut Peekable<I>)
    where
        I: Iterator<Item = &'a &'a str>,
    {
        let mut cd_path = PathBuf::new();
        if let Some(path) = self.get_path(iter) {
            cd_path.push(path);
        } else {
            cd_path.push(&self.home_directory);
        }
        match chdir(&cd_path) {
            Err(why) => eprintln!("Couldn't change directory: {}", why),
            Ok(_) => (),
        };
    }

    fn get_path<'a, I>(&self, iter: &mut Peekable<I>) -> Option<&'a &'a str>
    where
        I: Iterator<Item = &'a &'a str>,
    {
        iter.next_if(|&&x| x != "<" && x != ">" && x != "|" && x != ";")
    }

    fn handle_rangle<'a, I>(
        &self,
        args: &mut Vec<CString>,
        commands: &mut Vec<Command>,
        fd: &mut Option<RawFd>,
        iter: &mut Peekable<I>,
    ) where
        I: Iterator<Item = &'a &'a str>,
    {
        if let Some(filename) = self.get_path(iter) {
            if args.is_empty() {
                if let Some(command) = commands.last_mut() {
                    if let Some(fd1) = Shell::open_fd_for_write(filename) {
                        command.change_outfd(Some(fd1));
                    }
                }
            } else {
                let fd1 = Shell::open_fd_for_write(filename);
                let command = if let Some(fd0) = fd {
                    Command::from_fd(args.drain(..).collect::<Vec<CString>>(), Some(*fd0), fd1)
                } else {
                    Command::from_fd(args.drain(..).collect::<Vec<CString>>(), None, fd1)
                };
                commands.push(command);
            }
            *fd = None;
        } else {
            eprintln!("File Descriptor wasn't given");
        }
    }

    fn open_fd_for_write(filename: &str) -> Option<RawFd> {
        match open(
            filename,
            OFlag::O_WRONLY | OFlag::O_CREAT | OFlag::O_TRUNC,
            Mode::from_bits_truncate(0o644),
        ) {
            Ok(fd1) => Some(fd1),
            Err(why) => {
                eprintln!("Couldn't open {} as FileDescriptor: {}", filename, why);
                None
            }
        }
    }

    fn handle_langle<'a, I>(
        &self,
        args: &mut Vec<CString>,
        commands: &mut Vec<Command>,
        iter: &mut Peekable<I>,
    ) where
        I: Iterator<Item = &'a &'a str>,
    {
        match self.get_path(iter) {
            Some(filename) => {
                let command = Command::from_fd(
                    args.drain(..).collect::<Vec<CString>>(),
                    Shell::open_fd_for_read(filename),
                    None,
                );
                commands.push(command);
            }
            None => {
                eprintln!("File Descriptor wasn't given");
                args.clear();
            }
        };
    }

    fn open_fd_for_read(filename: &str) -> Option<RawFd> {
        match open(filename, OFlag::O_RDONLY, Mode::S_IRUSR) {
            Ok(fd0) => Some(fd0),
            Err(why) => {
                eprintln!("Couldn't open {} as FileDescriptor: {}", filename, why);
                None
            }
        }
    }

    fn handle_wildcard(&self, args: &mut Vec<CString>) {
        if let Ok(readdir) = fs::read_dir(&self.current_directory) {
            for entry in readdir {
                if let Ok(file) = entry {
                    args.push(CString::new(file.file_name().into_string().unwrap()).unwrap());
                }
            }
        }
    }

    fn handle_pipe(
        args: &mut Vec<CString>,
        commands: &mut Vec<Command>,
        fd: &mut Option<RawFd>,
        pipev: &mut Vec<(RawFd, RawFd)>,
    ) {
        let pipe = pipe().expect("Couldn't generate pipe");
        if args.len() > 0 {
            commands.push(Command::from_fd(args.clone(), fd.take(), Some(pipe.1)));
        } else {
            if let Some(command) = commands.last_mut() {
                command.change_outfd(Some(pipe.1));
            } else {
                eprintln!("There is no command before |")
            }
        }
        *fd = Some(pipe.0);
        args.clear();
        pipev.push(pipe);
    }

    fn handle_semicolon(
        args: &mut Vec<CString>,
        commands: &mut Vec<Command>,
        fd: &mut Option<RawFd>,
    ) {
        if args.len() > 0 {
            if let Some(fd0) = fd {
                commands.push(Command::from_fd(args.clone(), Some(*fd0), None));
            } else {
                commands.push(Command::new(args.clone()));
            }
            args.clear();
        }
    }
}
