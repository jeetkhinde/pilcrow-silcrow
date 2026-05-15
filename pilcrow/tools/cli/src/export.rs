use std::process::Command;

pub fn handle_export(args: &[String]) -> Result<(), String> {
    let dir = args.first().map(String::as_str).unwrap_or("dist");
    println!("exporting to {dir}...");
    let status = Command::new("cargo")
        .args(["run", "--", "export", dir])
        .status()
        .map_err(|e| format!("failed to run cargo: {e}"))?;
    if !status.success() {
        return Err(format!("cargo run exited with status: {status}"));
    }
    Ok(())
}
