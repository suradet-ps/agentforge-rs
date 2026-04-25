use std::fs;
use std::path::Path;

const AGENTS_FILE: &str = "AGENTS-RUST.md";

const CONTENT: &str = include_str!("../templates/AGENTS-RUST.md");

fn main() -> Result<(), Box<dyn std::error::Error>> {
  let target = Path::new(AGENTS_FILE);

  if target.exists() {
    println!("⚠️ {} already exists! Skipping installation", AGENTS_FILE);
    return Ok(());
  }

  fs::write(target, CONTENT)?;
  println!("✅ Created {} at the project root!", AGENTS_FILE);
  println!("🤖 The AI agent will automatically read this file.");
  Ok(())
}
