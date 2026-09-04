//! Builds the Next.js app in `web/` into `web/out/` (a static export) before
//! `src/assets.rs`'s `RustEmbed` derive scans that directory at compile time.
//! See docs/PLUGINS.md-adjacent reasoning in this crate's README for why this
//! lives in build.rs rather than being a separate manual step.

use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    let manifest_dir = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let web_dir = manifest_dir.join("web");
    let out_dir = web_dir.join("out");

    // Only re-run this script (and therefore only re-run `npm run build`,
    // which takes real time) when something that could change its output
    // actually changed -- not on every `cargo build`, and specifically not
    // because `npm run build` itself just wrote into web/out a moment ago.
    for watched in [
        "src",
        "public",
        "package.json",
        "package-lock.json",
        "next.config.mjs",
        "tsconfig.json",
    ] {
        println!("cargo:rerun-if-changed={}", web_dir.join(watched).display());
    }
    println!("cargo:rerun-if-env-changed=THAUMIEL_UI_SKIP_WEB_BUILD");

    if std::env::var_os("THAUMIEL_UI_SKIP_WEB_BUILD").is_some() {
        println!("cargo:warning=THAUMIEL_UI_SKIP_WEB_BUILD set -- using whatever is already in web/out, if anything");
        ensure_embeddable(&out_dir);
        return;
    }

    if !command_exists("npm") {
        println!(
            "cargo:warning=npm was not found on PATH -- skipping the Next.js build. \
             Run `npm install && npm run build` in crates/thaumiel-ui/web yourself, \
             or install Node.js, then rebuild. Falling back to a placeholder page for now."
        );
        ensure_embeddable(&out_dir);
        return;
    }

    if !web_dir.join("node_modules").is_dir() {
        run(&web_dir, "npm", &["install"]);
    }
    run(&web_dir, "npm", &["run", "build"]);

    ensure_embeddable(&out_dir);
}

fn command_exists(cmd: &str) -> bool {
    // `npm --version` rather than relying on a PATH-search helper crate --
    // this is the only thing build.rs needs it for.
    Command::new(npm_invocation(cmd))
        .arg("--version")
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

fn run(dir: &Path, cmd: &str, args: &[&str]) {
    let status = Command::new(npm_invocation(cmd))
        .args(args)
        .current_dir(dir)
        .status()
        .unwrap_or_else(|e| panic!("failed to spawn `{cmd} {}`: {e}", args.join(" ")));
    if !status.success() {
        panic!(
            "`{cmd} {}` exited with {status} -- see output above",
            args.join(" ")
        );
    }
}

// On Windows, `npm`/`npx` are `.cmd` shims; `Command::new("npm")` only finds
// them if `cmd.exe`'s PATHEXT resolution kicks in, which `std::process` does
// not do for you on all setups. Being explicit avoids an environment-
// dependent "program not found" that works for one contributor and not another.
#[cfg(target_os = "windows")]
fn npm_invocation(cmd: &str) -> String {
    format!("{cmd}.cmd")
}
#[cfg(not(target_os = "windows"))]
fn npm_invocation(cmd: &str) -> String {
    cmd.to_string()
}

/// `RustEmbed` needs *a* directory to scan at compile time. If the real
/// Next.js build didn't run (no npm, or explicitly skipped) and nothing was
/// ever built before, drop in a minimal placeholder so `cargo build` still
/// succeeds -- with a page that says exactly what's missing, rather than a
/// cryptic macro-expansion error.
fn ensure_embeddable(out_dir: &Path) {
    if out_dir.is_dir() {
        return;
    }
    std::fs::create_dir_all(out_dir).expect("failed to create web/out placeholder directory");
    std::fs::write(
        out_dir.join("index.html"),
        "<!doctype html><html><body style=\"background:#000;color:#ededed;font-family:sans-serif;\
         display:flex;align-items:center;justify-content:center;height:100vh;margin:0\">\
         <p>The Thaumiel UI hasn't been built yet.<br>Run <code>npm install &amp;&amp; npm run build</code> \
         in crates/thaumiel-ui/web, then rebuild.</p></body></html>",
    )
    .expect("failed to write placeholder index.html");
}
