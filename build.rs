//! Compiles the TFLY C core and links it into the crate.
//!
//! TFLY.h is a single-header library, so the only thing that needs compiling
//! is `native/tfly_ffi.c`, which includes the header once and exports a small
//! C ABI. The C sources are vendored in the repository, so a build works
//! offline.
//!
//! If no C compiler is available the crate still builds: the TFLY bindings
//! are then compiled out and [`crate::tfly`] reports the feature as absent,
//! rather than failing the whole build. The Rust-only parts of FlyTest, which
//! is everything else, do not depend on the C core.

use std::path::Path;
use std::process::Command;

/// Set `TFLY_NO_BUILD=1` to skip compiling the C core even if a compiler
/// exists. Useful when cross-compiling or when the C tests already ran.
fn skip_build() -> bool {
    std::env::var("TFLY_NO_BUILD").is_ok_and(|v| v == "1")
}

/// Return the first C compiler that responds to `--version`.
fn find_compiler() -> Option<String> {
    let candidates = if cfg!(target_os = "windows") {
        vec!["cc", "gcc", "clang", "clang-cl"]
    } else {
        vec!["cc", "gcc", "clang"]
    };
    for candidate in candidates {
        if Command::new(candidate)
            .arg("--version")
            .output()
            .map(|out| out.status.success())
            .unwrap_or(false)
        {
            return Some(candidate.to_owned());
        }
    }
    None
}

fn main() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ffi = root.join("native").join("tfly_ffi.c");
    let header = root.join("TFLY.h");

    println!("cargo:rerun-if-changed=native/tfly_ffi.c");
    println!("cargo:rerun-if-changed=TFLY.h");
    println!("cargo:rerun-if-env-changed=TFLY_NO_BUILD");
    println!("cargo:rerun-if-env-changed=CC");

    // The header is the model. If it is missing there is nothing to build and
    // no point pretending the bindings exist.
    if !header.exists() {
        println!("cargo:rustc-cfg=tfly_unavailable");
        println!("cargo:warning=TFLY.h not found; building without the TFLY core");
        return;
    }
    if skip_build() {
        println!("cargo:rustc-cfg=tfly_unavailable");
        println!("cargo:warning=TFLY_NO_BUILD=1; building without the TFLY core");
        return;
    }

    let compiler = std::env::var("CC").ok().or_else(find_compiler);
    let Some(compiler) = compiler else {
        println!("cargo:rustc-cfg=tfly_unavailable");
        println!("cargo:warning=no C compiler found; building without the TFLY core");
        return;
    };

    let out_dir_raw = std::env::var("OUT_DIR").expect("OUT_DIR is set by cargo");
    let out_dir = Path::new(&out_dir_raw);
    let object = out_dir.join("tfly_ffi.o");
    let archive = out_dir.join("libtfly_ffi.a");
    let out_dir_display = out_dir_raw.clone();

    let mut args: Vec<String> = vec!["-std=c99".into(), "-O2".into(), "-c".into()];
    if cfg!(target_os = "windows") {
        // Do not require the GNU runtime DLLs to be on PATH.
        args.push("-fno-strict-aliasing".into());
    }
    args.push(ffi.display().to_string());
    args.push("-o".into());
    args.push(object.display().to_string());

    let status = Command::new(&compiler)
        .args(&args)
        .current_dir(root)
        .status()
        .unwrap_or_else(|e| panic!("failed to run {compiler}: {e}"));

    if !status.success() {
        println!("cargo:rustc-cfg=tfly_unavailable");
        println!("cargo:warning=failed to compile TFLY core; building without it");
        return;
    }

    // GNU ar works the same way on the toolchains this project targets.
    let archived = Command::new("ar")
        .arg("crs")
        .arg(&archive)
        .arg(&object)
        .current_dir(root)
        .status()
        .map(|s| s.success())
        .unwrap_or(false);

    if archived {
        println!("cargo:rustc-link-search=native={out_dir_display}");
        println!("cargo:rustc-link-lib=static=tfly_ffi");
        println!("cargo:rustc-cfg=tfly_available");
    } else {
        println!("cargo:rustc-cfg=tfly_unavailable");
        println!("cargo:warning=ar failed; building without the TFLY core");
    }

    // libm is needed by expf, sqrtf, cos and friends inside the header.
    if cfg!(unix) {
        println!("cargo:rustc-link-lib=m");
    }
}
