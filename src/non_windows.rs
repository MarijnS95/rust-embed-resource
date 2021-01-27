use std::process::{Command, Stdio};
use std::path::PathBuf;

#[derive(Debug, Clone)]
enum ToolKind {
    /// LLVM-rc. Note that LLVM-RC requires a separate C preprocessor to
    /// preprocess the rc file.
    LlvmRc { rc: String },
    /// MinGW windres.
    WindRes { exec: String },
}

#[derive(Debug, Clone)]
pub struct ResourceCompiler {
    tool: Option<ToolKind>,
}


impl ResourceCompiler {
    pub fn new() -> ResourceCompiler {
        ResourceCompiler { tool: find_rc_tool() }
    }

    #[inline]
    pub fn is_supported(&self) -> bool {
        match std::env::var("TARGET").as_deref() {
            Ok("x86_64-pc-windows-msvc") => true,
            Ok("i686-pc-windows-msvc") => true,
            Ok("x86_64-pc-windows-gnu") => true,
            Ok("i686-pc-windows-gnu") => true,
            _ => false,
        }
    }

    pub fn compile_resource(&self, out_dir: &str, prefix: &str, resource: &str) {
        let kind = self.tool.as_ref().expect("Couldn't find windres or llvm-rc. Make sure one of them is in your $PATH.");

        match kind {
           ToolKind::WindRes { exec } => compile_windres(&exec, out_dir, prefix, resource),
           ToolKind::LlvmRc { rc } => compile_llvm_rc(&rc, out_dir, prefix, resource),
        }
    }
}

fn compile_llvm_rc(rc_exec: &str, out_dir: &str, prefix: &str, resource: &str) {
    // First, we have to run cpp on the resource file as it doesn't
    let expanded = cc::Build::new()
        .define("RC_INVOKED", None)
        .file(resource)
        .cargo_metadata(false)
        .expand();

    let out_file = format!("{}/{}.preprocessed.rc", out_dir, prefix);
    std::fs::write(&out_file, expanded).unwrap();

    if !Command::new(rc_exec)
        .args(&["/fo", &format!("{}/{}.lib", out_dir, prefix)])
        .arg(out_file)
        .stdin(Stdio::piped())
        .status()
        .expect(&format!("Failed to run {}.", rc_exec))
        .success()
    {
        panic!("{} failed to compile the resource file.", rc_exec);
    }
}

fn compile_windres(exec: &str, out_dir: &str, prefix: &str, resource: &str) {
    let out_file = format!("{}/lib{}.a", out_dir, prefix);
    match Command::new(exec).args(&["--input", resource, "--output-format=coff", "--output", &out_file]).status() {
        Ok(stat) if stat.success() => {}
        Ok(stat) => panic!("{} failed to compile \"{}\" into \"{}\" with {}", exec, resource, out_file, stat),
        Err(e) => panic!("Couldn't to execute {} to compile \"{}\" into \"{}\": {}", exec, resource, out_file, e),
    }
}

fn command_exists(s: &str) -> bool {
    match Command::new(s).spawn() {
        Ok(mut v) => { let _ = v.kill(); true },
        Err(_err) => false,
    }
}

fn detect_tool_kind(s: &str) -> ToolKind {
    // -V will print the version in windres. /? will print the help in llvm-rc
    // and microsoft rc. They can be combined, /? takes precedence over -V.
    let out = match Command::new(s).args(&["-V", "/?"]).output() {
        Ok(v) => v,
        Err(err) => panic!("Failed to run {}: {}", s, err)
    };

    if out.stdout.starts_with(b"GNU windres") {
        ToolKind::WindRes { exec: s.into() }
    } else if out.stdout.starts_with(b"OVERVIEW: Resource Converter") {
        ToolKind::LlvmRc { rc: s.into() }
    } else {
        panic!("Unknown RC program version found at path: {}", s)
    }
}

fn find_rc_tool() -> Option<ToolKind> {
    let target = std::env::var("TARGET").ok()?;

    // If there's an RC binary explicitly set in an environment variable, use
    // that.
    if let Some(rc) = get_var("RC") {
        let kind = detect_tool_kind(&rc);
        return Some(kind)
    }

    // Otherwise, try to autodetect based on target.
    if target == "x86_64-pc-windows-gnu" && command_exists("x86_64-w64-mingw32-windres") {
        Some(ToolKind::WindRes { exec: "x86_64-w64-mingw32-windres".into() })
    } else if target == "i686-pc-windows-gnu" && command_exists("i686-w64-mingw32-windres") {
        Some(ToolKind::WindRes { exec: "i686-w64-mingw32-windres".into() })
    } else {
        None
    }
}


/// Get a target-specific environment variable based on the passed value. This
/// is used to find the appropriate tool for a given target: When
/// cross-compiling to windows `x86_64-pc-windows-msvc`, we will look for
/// environments variables like `RC_x86_64-pc-windows-msvc`
fn get_var(var_base: &str) -> Option<String> {
    let target = std::env::var("TARGET").unwrap();
    let host = std::env::var("HOST").unwrap();
    let kind = if host == target { "HOST" } else { "TARGET" };
    let target_u = target.replace("-", "_");
    std::env::var(&format!("{}_{}", var_base, target))
        .or_else(|_| std::env::var(&format!("{}_{}", var_base, target_u)))
        .or_else(|_| std::env::var(&format!("{}_{}", kind, var_base)))
        .or_else(|_| std::env::var(var_base))
        .ok()
}

pub fn find_windows_sdk_tool_impl(_: &str) -> Option<PathBuf> {
    None
}
