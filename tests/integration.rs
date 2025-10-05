use std::env;
use std::env::current_dir;
use std::error::Error;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

// adapted from `assert_cmd` crate
fn target_dir() -> PathBuf {
    env::current_exe()
        .ok()
        .map(|mut path| {
            path.pop();
            if path.ends_with("deps") {
                path.pop();
            }
            path
        })
        .unwrap()
}

// https://stackoverflow.com/a/35907071/2241008
fn find_subsequence(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Makes a path object, and checks that it exists.
fn make_path_and_check(
    tests_dir: &Path,
    path: &[&str],
    object: &str,
    is_executable: bool,
) -> PathBuf {
    let mut buf = tests_dir.to_path_buf();
    if is_executable {
        for part in &path[..(path.len() - 1)] {
            buf.push(part);
        }
        buf.push(format!(
            "{}{}",
            path.last().unwrap(),
            env::consts::EXE_SUFFIX
        ));
        println!("{}", buf.iter().last().unwrap().display())
    } else {
        for part in path {
            buf.push(part);
        }
    }

    if !buf.exists() {
        panic!(
            "Couldn't find {} at {}. Please see {} for more details.",
            object,
            buf.display(),
            tests_dir.join("README.md").display()
        );
    }
    buf
}

fn build_object<I: Iterator<Item = P>, P: AsRef<OsStr>>(
    tests_dir: &Path,
    output_name: &Path,
    sources: I,
    extra_compile_args: &[&str],
) -> Result<(), Box<dyn Error>> {
    let clang_path = make_path_and_check(tests_dir, &["llvm", "bin", "clang"], "Clang", true);

    let bin_path = make_path_and_check(
        tests_dir,
        &["common-3.0.sdk", "usr", "bin"],
        "binary directory",
        false,
    );

    let sdk_path = make_path_and_check(tests_dir, &["common-3.0.sdk"], "SDK sysroot", false);

    let mut linker_arg = OsString::from("-B");
    linker_arg.push(bin_path);
    let mut sdk_arg = OsString::from("--sysroot=");
    sdk_arg.push(sdk_path);

    eprintln!("Building {} for iPhone OS 3...", output_name.display());
    std::io::stderr().flush().unwrap();
    let mut cmd = Command::new(clang_path);
    let output = cmd
        // Uncomment for verbose output (useful for debugging search path
        // issues)
        // .arg("-v")
        // Uncomment for verbose linker output
        // .arg("-Wl,-v")
        // Target iPhone OS 2
        .arg("--target=arm-apple-ios")
        .arg("-miphoneos-version-min=2.0")
        .args(["-arch", "armv7"])
        // If enabled, the stack protection causes a null pointer crash in some
        // functions. This is probably because ___stack_chk_guard isn't linked.
        .arg("-fno-stack-protector")
        .arg("-DPRODUCT_iPhone")
        .arg(linker_arg)
        .arg(sdk_arg)
        .args(extra_compile_args)
        // Input files.
        .args(sources)
        // Write the output to the bundle.
        .arg("-o")
        .arg(output_name)
        .output()
        .expect("failed to execute Clang process");
    eprintln!("Running {:?}", cmd);
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();
    assert!(output.status.success());
    eprintln!("Built successfully.");
    Ok(())
}

// Note that source files are looked for in the path
// "{tests_dir}/{test_app_name}_source"
// and binaries are output as
// "{tests_dir}/{test_app_name}.app/{test_app_name}".
fn run_test_app(
    tests_dir: &Path,
    test_app_name: &str,
    sources: &[&Path],
    extra_compile_args: &[&str],
    extra_run_args: &[&str],
) -> Result<(), Box<dyn Error>> {
    let test_app_path = tests_dir.join(format!("{}.app", test_app_name));
    build_object(
        &tests_dir,
        &tests_dir
            .join(format!("{}.app", test_app_name))
            .join(test_app_name),
        sources.iter().map(|file| {
            tests_dir
                .join(format!("{}_source", test_app_name))
                .join(file)
        }),
        extra_compile_args,
    )?;
    let binary_name = "touchHLE";
    let binary_path = target_dir().join(format!("{}{}", binary_name, env::consts::EXE_SUFFIX));
    let mut cmd = Command::new(binary_path);
    let output = cmd
        .arg(test_app_path)
        // headless mode avoids a distracting window briefly appearing during
        // testing, and works in CI.
        .arg("--headless")
        .args(extra_run_args)
        .output()
        .expect("failed to execute touchHLE process");
    std::io::stdout().write_all(&output.stdout).unwrap();
    std::io::stderr().write_all(&output.stderr).unwrap();
    assert!(output.status.success());
    // sanity check: check that emulation actually happened
    assert_ne!(
        find_subsequence(output.stderr.as_slice(), b"CPU emulation begins now."),
        None
    );
    write!(
        &mut std::io::stdout(),
        "Finished running {}.\n\n\n",
        test_app_name
    )
    .unwrap();
    Ok(())
}

#[test]
fn test_app() -> Result<(), Box<dyn Error>> {
    let tests_dir = current_dir()?.join("tests");
    let stubs_dir = tests_dir.join("stubs");

    // Wipe and recreate the stubs dir to ensure it is clean.
    let _ = std::fs::remove_dir_all(&stubs_dir);
    std::fs::create_dir(&stubs_dir).unwrap();

    let sdk_libs_dir_arg =
        "-L".to_owned() + current_dir()?.join("touchHLE_dylibs").to_str().unwrap();
    let stubs_dir_arg = "-L".to_owned() + stubs_dir.to_str().unwrap();
    let mut extra_linker_args = Vec::<String>::new();
    let mut extra_compile_args = vec![
        "-mlinker-version=253",
        "-Wno-expansion-to-defined",
        "-Wno-literal-range",
        sdk_libs_dir_arg.as_str(),
        stubs_dir_arg.as_str(),
        "-ObjC",
        "-fno-objc-exceptions",
        // ARC is not available until IOS 5, so it can't be used.
        "-fno-objc-arc",
        "-fno-objc-arc-exceptions",
    ];

    // Generate symbols.
    let symbols_path = stubs_dir.join("SYMBOLS.txt");
    let dump_file_option = format!("--dump-file={}", symbols_path.to_str().unwrap());
    let dump_run_args = ["--dump=symbols", dump_file_option.as_str(), "--headless"];
    let binary_name = "touchHLE";
    let binary_path = target_dir().join(format!("{}{}", binary_name, env::consts::EXE_SUFFIX));
    let mut cmd = Command::new(binary_path);
    let output = cmd
        .args(dump_run_args)
        .output()
        .expect("failed to execute touchHLE process");
    assert!(output.status.success());

    let mut files_to_compile = Vec::<(String, PathBuf)>::new();
    {
        let mut in_body = false;
        let mut current_file = None::<BufWriter<File>>;
        for line in BufReader::new(File::open(symbols_path).unwrap()).lines() {
            let line = line.unwrap();
            if let Some(dylib_path) = line.strip_prefix("// ") {
                // First comment after a series of non-comment lines, or first
                // comment in the file: this is the canonical name of the dylib.
                if in_body || current_file.is_none() {
                    let dylib_name = dylib_path.rsplit_once("/").unwrap().1;
                    let stub_src_path = stubs_dir.join(format!("{}.m", dylib_name));
                    current_file = Some(BufWriter::new(File::create(&stub_src_path).unwrap()));
                    files_to_compile.push((dylib_path.to_string(), stub_src_path));
                    in_body = false;
                }
                // Ignore the non-canonical dylib names for now.
            } else if let Some(ref mut current_file) = current_file {
                in_body = true;
                writeln!(current_file, "{}", line).unwrap();
            }
        }
        // current_file dropping out of scope here flushes it.
    }

    for (dylib_path, stub_src_path) in files_to_compile {
        if dylib_path.starts_with("/.touchHLE") {
            // skip the fake app picker library
            continue;
        }
        let compile_args = [
            "-mlinker-version=253",
            "-fno-builtin",
            "-nostdlib",
            &format!("-Wl,-install_name,{}", dylib_path),
            "-Wno-objc-root-class", // silence clang warning about inheritance
            "-Wl,-dylib",
            "-lobjc",
        ];
        let dylib_name = dylib_path.rsplit_once("/").unwrap().1;
        // World's most horrible heuristic:
        // - if the name begins with 'lib' (libSystem, libobjc) then it should
        //   not link against libobjc, and it will (hopefully) be compiled
        //   before any normal Objective-C code. We need to strip the 'lib' and
        //   '.dylib' parts of its filename to get something we can pass to '-l'
        //   (i.e. 'libobjc.dylib' -> '-lobjc')
        // - if the name does not begin with 'lib', then it's probably a
        //   framework, and we need to ensure the test app links against it with
        //   '-l', and to that end we need to add the 'lib' and '.dylib' parts
        //   to its filename, again so that that '-l' will find it
        //   (i.e. 'FooBarKit' -> 'libFooBarKit.dylib')
        let (compile_args, out_path) = if let Some(bare_name) = dylib_name.strip_prefix("lib") {
            extra_linker_args.push(format!("-l{}", bare_name.strip_suffix(".dylib").unwrap()));
            (
                &compile_args[..compile_args.len() - 1], // skip "-lobjc"
                stubs_dir.join(dylib_name),
            )
        } else {
            extra_linker_args.push(format!("-l{dylib_name}"));
            (
                &compile_args[..], // include "-lobjc"
                stubs_dir.join(format!("lib{dylib_name}.dylib")),
            )
        };
        build_object(&tests_dir, &out_path, [stub_src_path].iter(), &compile_args).unwrap();
    }

    // Vec<String> -> &[&str] ownership shenanigans
    for arg in &extra_linker_args {
        extra_compile_args.push(&arg);
    }

    let sources = ["main.m", "SyncTester.m"].map(|file| Path::new(file));
    run_test_app(&tests_dir, "TestApp", &sources, &extra_compile_args, &[])
}
