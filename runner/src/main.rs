use std::{
    fs::File,
    io::IsTerminal,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::Duration,
};

use bootloader::{BiosBoot, UefiBoot};
use clap::Parser;
use cpio::{NewcBuilder, write_cpio};
use humansize::{BINARY, DECIMAL, SizeFormatter};
use qemu_common::{KERNEL_START, QemuExitCode};
use serde::Deserialize;
use walkdir::WalkDir;

use crate::disk::BootBuilder;

mod disk;

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
    #[arg(env = "NO_DISPLAY", long)]
    no_display: bool,
    #[arg(default_value = "target/ovmf", long, env = "OVMF_PREBUILT_DIR")]
    ovmf_prebuilt: PathBuf,
    #[arg(long, env = "INITRD_DIR")]
    initrd: Option<PathBuf>,
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

fn path_as_blog_os_path<P: AsRef<Path>>(path: P) -> String {
    path.as_ref()
        .components()
        .map(|c| c.as_os_str().display().to_string())
        .collect::<Vec<_>>()
        .join("/")
}

fn main() {
    let args = Args::parse();
    let uefi = !args.no_uefi;
    let target = dunce::canonicalize(
        args.target
            .or_else(get_env_target_dir)
            .or_else(get_manifest_target_dir)
            .unwrap_or_else(|| PathBuf::from("target")),
    )
    .unwrap();

    // for (var, val) in std::env::vars() {
    //     println!("ENV: {var}={val}");
    // }

    // choose whether to start the UEFI or BIOS image

    let kernel = dunce::canonicalize(args.kernel).unwrap();
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

    println!("   cwd: {:?}", std::env::current_dir());
    println!(" build: {}", args.build);
    println!("   gdb: {}", args.gdb);
    println!("  uefi: {uefi}");
    println!("target: {}", target.display());
    println!("kernel: {}", kernel.display());
    println!("   out: {}", out_dir.display());
    println!("  ovmf: {}", args.ovmf_prebuilt.display());
    println!("initrd: {:?}", args.initrd);
    println!("prefix: {prefix}");

    std::fs::create_dir_all(&out_dir).unwrap();

    let cpio = args.initrd.map(|initrd| {
        let path = target.join("initrd.cpio");

        let file = File::create(&path).unwrap();

        println!("walking initrd {}", initrd.display());

        let names_paths = WalkDir::new(&initrd)
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .map(|entry| {
                (
                    path_as_blog_os_path(entry.path().strip_prefix(&initrd).unwrap()),
                    entry.into_path(),
                )
            })
            .collect::<Vec<_>>();
        write_cpio(
            names_paths
                .iter()
                .inspect(|(name, path)| println!("[INITRD] {name} at {}", path.display()))
                .map(|(name, path)| (NewcBuilder::new(name), File::open(path).unwrap())),
            file,
        )
        .unwrap();


        let stat = path.metadata().unwrap();

        println!("Wrote cpio to {}: {}/{} ({} bytes)", path.display(), SizeFormatter::new(stat.len(), DECIMAL), SizeFormatter::new(stat.len(), BINARY), stat.len());

        path
    });

    let mut boot_builder: Box<dyn BootBuilder> = if uefi {
        Box::new(UefiBoot::new(&kernel))
    } else {
        Box::new(BiosBoot::new(&kernel))
    };

    boot_builder.set_ramdisk_opt(cpio.as_deref());

    let path = if uefi {
        out_dir.join(format!("{prefix}uefi.img"))
    } else {
        // create a BIOS disk image

        out_dir.join(format!("{prefix}bios.img"))
    };

    boot_builder.create_disk_image(&path).unwrap();

    let stat = path.metadata().unwrap();

    println!("Built at {} with size: {}/{} ({} bytes)", path.display(), SizeFormatter::new(stat.len(), DECIMAL), SizeFormatter::new(stat.len(), BINARY), stat.len());

    if !args.build {
        println!("Running qemu");

        let mut cmd = std::process::Command::new("qemu-system-x86_64");
        if args.gdb {
            cmd.arg("-s").arg("-S");
        }

        if uefi {
            println!("Downloading OVMF");
            let ovmf =
                ovmf_prebuilt::Prebuilt::fetch(ovmf_prebuilt::Source::LATEST, args.ovmf_prebuilt)
                    .unwrap();
            let ovmf_code = ovmf.get_file(ovmf_prebuilt::Arch::X64, ovmf_prebuilt::FileType::Code);
            let ovmf_vars = ovmf.get_file(ovmf_prebuilt::Arch::X64, ovmf_prebuilt::FileType::Vars);
            println!(
                "Downloaded OVMF: {} & {}",
                ovmf_code.display(),
                ovmf_vars.display()
            );
            cmd.arg("-drive").arg(format!(
                "if=pflash,format=raw,readonly=on,file={}",
                ovmf_code.display()
            ));
            cmd.arg("-drive")
                .arg(format!("if=pflash,format=raw,file={}", ovmf_vars.display()));
        }
        cmd.arg("-drive")
            .arg(format!("format=raw,file={}", path.display()));
        cmd.arg("-device")
            .arg("isa-debug-exit,iobase=0xf4,iosize=0x04");
        cmd.arg("-device").arg("pci-serial");
        cmd.arg("-device").arg("pci-serial");

        cmd.arg("-serial").arg("stdio");
        cmd.arg("-serial").arg("file:out.json.log");

        if is_test || (args.no_display && (!args.gdb || args.no_start_gdb)) {
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
                "add-symbol-file {:?} -o 0x{:x}",
                kernel.display(),
                KERNEL_START
            ));
            cmd.arg("-ex").arg("set disassemble-next-line on");
            // cmd.arg("-ex").arg("display /-16i $pc");
            // cmd.arg("-ex").arg("display /16i $pc");

            cmd.stdin(Stdio::inherit());

            cmd.stdout(Stdio::inherit());

            let is_terminal = std::io::stdin().is_terminal();
            println!("stdin is terminal: {is_terminal}");

            println!("Running {cmd:?}");

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
