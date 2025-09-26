use std::{path::PathBuf, process::{Command, Stdio}};

use clap::Parser;
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
    kernel: PathBuf
}

fn get_env_target_dir() -> Option<PathBuf> {
    std::env::var("CARGO_TARGET_DIR").ok().map(PathBuf::from)
}

#[derive(Debug, Deserialize)]
struct LocateProjectOut {
    root: PathBuf
}

fn get_manifest_target_dir() -> Option<PathBuf> {
    let mut command = Command::new(std::env::var("CARGO").unwrap_or("cargo".into()));
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
    let target = args.target.or_else(get_env_target_dir).or_else(get_manifest_target_dir).unwrap_or_else(|| PathBuf::from("target")).canonicalize().unwrap();


    // choose whether to start the UEFI or BIOS image

    let kernel = args.kernel.canonicalize().unwrap();


    let (out_dir, prefix) = if kernel.starts_with(&target) {
        // Same target found
        let out_dir = kernel.parent().unwrap().join("disk_images");
        let prefix = kernel.file_prefix().map(|x| x.to_string_lossy().into_owned() + "_").unwrap_or_default();
        (out_dir, prefix)
    } else {
        (target.join("disk_images"), String::new())
    };


    println!(" build: {}", args.build);
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
    }else{
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
        if uefi {
            cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
            cmd.arg("-drive")
                .arg(format!("format=raw,file={}", path.display()));
        } else {
            cmd.arg("-drive")
                .arg(format!("format=raw,file={}", path.display()));
        }
        cmd.stdin(Stdio::null());
        cmd.stdout(Stdio::inherit());
        cmd.stderr(Stdio::inherit());
        let mut child = cmd.spawn().unwrap();
        child.wait().unwrap();
    }


    
}
