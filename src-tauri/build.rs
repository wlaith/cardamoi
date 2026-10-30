fn main() {
    // Embed the TMDB token from src-tauri/.env (kept out of git) at compile time.
    println!("cargo:rerun-if-changed=.env");
    println!("cargo:rerun-if-env-changed=TMDB_TOKEN");
    let token = std::env::var("TMDB_TOKEN").ok().or_else(|| {
        std::fs::read_to_string(".env").ok()?.lines().find_map(|line| {
            let value = line.trim().strip_prefix("TMDB_TOKEN=")?;
            Some(value.trim_matches(|c| c == '"' || c == '\'').to_owned())
        })
    });
    if let Some(token) = token.filter(|t| !t.is_empty()) {
        println!("cargo:rustc-env=TMDB_TOKEN={token}");
    }

    tauri_build::build()
}
