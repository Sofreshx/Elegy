use serde_json::{json, Value};
use std::io::{self, Read};
use std::time::Duration;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = std::env::args().skip(1).collect::<Vec<_>>();
    if args.iter().any(|arg| arg == "--malformed") {
        println!("not-json");
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--large") {
        println!("{}", "x".repeat(1024));
        return Ok(());
    }
    if args.iter().any(|arg| arg == "--sleep") {
        std::thread::sleep(Duration::from_secs(1));
    }

    let mut input = String::new();
    io::stdin().read_to_string(&mut input)?;
    let value: Value = serde_json::from_str(&input)?;
    println!("{}", json!({"echo": value["value"]}));
    Ok(())
}
