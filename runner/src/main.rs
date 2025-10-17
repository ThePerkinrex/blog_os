use std::{
    io::{IsTerminal, Read},
    path::PathBuf,
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use bootloader::BootConfig;
use clap::Parser;
use qemu_common::QemuExitCode;
use serde::Deserialize;

#[derive(Parser)]
#[command(version, about, long_about = None)]
struct Args {
    #[arg(env = "RUNNER_NO_UEFI", long)]
    no_uefi: bool,
    #[arg(long)]
    build: bool,
    #[arg(long)]
    target: Option<PathBuf>,
    kernel: PathBuf,
    #[arg(env = "GDB_LISTEN", long)]
    gdb: bool,
    #[arg(env = "NO_START_GDB", long)]
    no_start_gdb: bool,
}

fn get_env_target_dir() -> Option<PathBuf> {
    std::env::var("CARGO_TARGET_DIR").ok().map(PathBuf::from)
}

#[derive(Debug, Deserialize)]
struct LocateProjectOut {
    root: PathBuf,
}

fn get_manifest_target_dir() -> Option<PathBuf> {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or_else(|_| "cargo".into()));
    command.arg("locate-project");

    if let Ok(manifest_path) = std::env::var("CARGO_MANIFEST_PATH") {
        command.arg("--manifest-path").arg(manifest_path);
    }
    command.arg("--workspace");
    let out = command.output().ok()?;
    let location: LocateProjectOut = serde_json::from_slice(&out.stdout).ok()?;
    Some(location.root.parent()?.join("target"))
}

fn main() {
    let args = Args::parse();
    let uefi = !args.no_uefi;
    let target = args
        .target
        .or_else(get_env_target_dir)
        .or_else(get_manifest_target_dir)
        .unwrap_or_else(|| PathBuf::from("target"))
        .canonicalize()
        .unwrap();

    // for (var, val) in std::env::vars() {
    //     println!("ENV: {var}={val}");
    // }

    // choose whether to start the UEFI or BIOS image

    let kernel = args.kernel.canonicalize().unwrap();
    let kernel_parent = kernel.parent().expect("kernel parent");

    let is_doctest = kernel_parent
        .file_name()
        .expect("kernel executable's parent has no file name")
        .to_str()
        .expect("kernel executable's parent file name is not valid UTF-8")
        .starts_with("rustdoctest");
    let is_test = is_doctest || kernel_parent.ends_with("deps");

    let (out_dir, prefix) = if kernel.starts_with(&target) {
        // Same target found
        let out_dir = kernel_parent.join("disk_images");
        let prefix = kernel
            .file_prefix()
            .map(|x| x.to_string_lossy().into_owned() + "_")
            .unwrap_or_default();
        (out_dir, prefix)
    } else {
        (target.join("disk_images"), String::new())
    };

    println!(" build: {}", args.build);
    println!("   gdb: {}", args.gdb);
    println!("  uefi: {uefi}");
    println!("target: {}", target.display());
    println!("kernel: {}", kernel.display());
    println!("   out: {}", out_dir.display());
    println!("prefix: {prefix}");

    std::fs::create_dir_all(&out_dir).unwrap();

    let path = if uefi {
        let uefi_path = out_dir.join(format!("{prefix}uefi.img"));
        bootloader::UefiBoot::new(&kernel)
            .create_disk_image(&uefi_path)
            .unwrap();
        uefi_path
    } else {
        // create a BIOS disk image
        let bios_path = out_dir.join(format!("{prefix}bios.img"));
        bootloader::BiosBoot::new(&kernel)
            .create_disk_image(&bios_path)
            .unwrap();
        bios_path
    };

    println!("Built at {}", path.display());
    if !args.build {
        println!("Running qemu");

        let mut cmd = std::process::Command::new("qemu-system-x86_64");
        if args.gdb {
            cmd.arg("-s").arg("-S");
        }

        if uefi {
            cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        }
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", path.display()));
        cmd.arg("-device")
            .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");

        cmd.arg("-serial").arg("stdio");

        if is_test {
            cmd.arg("-display").arg("none");
        } else {
            #[cfg(target_os = "linux")]
            cmd.arg("-display").arg("sdl");
        }

        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());

        println!("Running {cmd:?}");

        let mut child = cmd.spawn().unwrap();

        if args.gdb && !args.no_start_gdb {
            thread::sleep(Duration::from_secs(1));
            let mut cmd = Command::new("gdb");
            cmd.arg("-ex").arg("target remote localhost:1234");
            cmd.arg("-ex").arg(format!(
                "add-symbol-file \"{}\" -o 0x8000000000",
                kernel.display()
            ));
            cmd.arg("-ex").arg("set  disassemble-next-line on");
            // cmd.arg("-ex").arg("display /-16i $pc");
            // cmd.arg("-ex").arg("display /16i $pc");

            cmd.stdin(Stdio::inherit());

            cmd.stdout(Stdio::inherit());

            let is_terminal = std::io::stdin().is_terminal();
            println!("stdin is terminal: {is_terminal}");

            // let mut line = String::new();
            // let read = std::io::stdin().read_line(&mut line).unwrap();
            // println!("Read ({read}): {line:?}");

            let mut gdb = cmd.spawn().unwrap();
            gdb.wait().unwrap();
            child.kill().unwrap();
        }

        // TODO add test run timeout

        let status = child.wait().unwrap();
        let exit_code = match status.code() {
            None => {
                println!("No exit code");
                10
            }
            Some(0) => {
                println!("qemu closed");
                0
            }
            Some(x) if x as u32 == ((QemuExitCode::Success as u32) << 1 | 1) => {
                println!("SUCCESS");
                0
            }
            Some(x) if x as u32 == ((QemuExitCode::Failed as u32) << 1 | 1) => {
                println!("FAILED");
                1
            }
            Some(x) if x as u32 == ((QemuExitCode::PanicWriterFailed as u32) << 1 | 1) => {
                println!("Panicked and the writer failed");
                2
            }
            Some(x) => {
                println!("Unknown exit code: {x} 0x{x:x}");
                3
            }
        };

        std::process::exit(exit_code);
    }
}
