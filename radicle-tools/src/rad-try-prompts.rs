use radicle_cli::terminal;

fn main() -> anyhow::Result<()> {
    let fruit = terminal::io::select(
        "Enter your favorite fruit:",
        &["apple", "pear", "banana", "strawberry"],
        &"apple",
    )?;

    if let Some(fruit) = fruit {
        terminal::success!("You have chosen '{fruit}'");
    } else {
        terminal::info!("Ok, bye.");
    }

    Ok(())
}
