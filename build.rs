fn main() {
    println!("cargo:rerun-if-changed=.git/HEAD");
    if let Some(rev) = rev_parse() {
        println!("cargo:rustc-env=RUSTBOT_REV={}", rev);
    }
}

/// Retrieves SHA-1 git revision. Returns `None` if any step of the way fails,
/// since this is only nice to have for the ?revision command and shouldn't fail builds.
fn rev_parse() -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["rev-parse", "--short=9", "HEAD"])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}
