use nix::{
    libc::{STDIN_FILENO, STDOUT_FILENO},
    sys::{
        signal::{kill, SIGTERM},
        wait::{waitpid, WaitStatus},
    },
    unistd::{dup2, execvp, fork, getpid, ForkResult},
};
use std::ffi::CString;
use std::os::fd::RawFd;

#[derive(Debug)]
pub struct Command {
    argv: Vec<CString>,
    infd: Option<RawFd>,
    outfd: Option<RawFd>,
}

impl Command {
    pub fn new(argv: Vec<CString>) -> Self {
        Self {
            argv,
            infd: None,
            outfd: None,
        }
    }

    pub fn get_infd(&self) -> &Option<RawFd> {
        &self.infd
    }

    pub fn get_outfd(&self) -> &Option<RawFd> {
        &self.outfd
    }

    pub fn from_fd(argv: Vec<CString>, infd: Option<RawFd>, outfd: Option<RawFd>) -> Self {
        Self { argv, infd, outfd }
    }

    pub fn change_outfd(&mut self, outfd: Option<RawFd>) {
        self.outfd = outfd;
    }

    pub fn execute(&self) {
        match unsafe { fork() }.expect("fork failed") {
            ForkResult::Parent { child } => {
                // println!("parent {}", std::process::id());
                match waitpid(child, None).expect("waitpid failed") {
                    WaitStatus::Exited(pid, status) => {
                        println!("Exit: pid={:?}, status={:?}", pid, status)
                    }
                    WaitStatus::Signaled(pid, status, _) => {
                        println!("Signal: pid={:?}, status={:?}", pid, status)
                    }
                    _ => eprintln!("Other waitstatus"),
                }
            }
            ForkResult::Child => {
                // println!("child {}", std::process::id());
                if let Some(fd) = self.infd {
                    dup2(fd, STDIN_FILENO).expect("Duplicate fd1 to fd2 failed");
                }
                if let Some(fd) = self.outfd {
                    dup2(fd, STDOUT_FILENO).expect("Duplicate fd1 to fd2 failed");
                }
                match execvp(&self.argv[0], &self.argv) {
                    Err(why) => {
                        eprintln!("Execute command failed: {}", why);
                        match kill(getpid(), SIGTERM) {
                            Ok(_) => {}
                            Err(why) => {
                                eprintln!("Couldn't terminate child: {}", why);
                            }
                        }
                    }
                    Ok(_) => {}
                }
            }
        }
    }
}
