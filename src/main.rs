mod shell;

fn main() {
    let mut ishell = shell::Shell::new();
    ishell.run();
}
