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
                Command::handle_waitstatus(waitpid(child, None).expect("waitpid failed"))
            }
            ForkResult::Child => {
                // println!("child {}", std::process::id());
                self.duplicate_fd();
                match execvp(&self.argv[0], &self.argv) {
                    Err(why) => {
                        eprintln!("Execute command failed: {}", why);
                        if let Err(why) = kill(getpid(), SIGTERM) {
                            eprintln!("Couldn't terminate child: {}", why);
                        }
                    }
                    Ok(_) => {}
                }
            }
        }
    }

    fn handle_waitstatus(waitstatus: WaitStatus) {
        // println!("parent {}", std::process::id());
        match waitstatus {
            WaitStatus::Exited(pid, status) => {
                println!("Exit: pid={:?}, status={:?}", pid, status)
            }
            WaitStatus::Signaled(pid, status, _) => {
                println!("Signal: pid={:?}, status={:?}", pid, status)
            }
            _ => eprintln!("Other waitstatus"),
        }
    }

    fn duplicate_fd(&self) {
        if let Some(fd) = self.infd {
            dup2(fd, STDIN_FILENO).expect("Duplicate input to stdin failed");
        }
        if let Some(fd) = self.outfd {
            dup2(fd, STDOUT_FILENO).expect("Duplicate output to stdout failed");
        }
    }
}
