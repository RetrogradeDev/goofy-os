fn main() {
    // read env variables that were set in build script
    let uefi_path = env!("UEFI_PATH");
    let bios_path = env!("BIOS_PATH");

    // choose whether to start the UEFI or BIOS image
    let uefi = false; // change to true to use UEFI

    let mut cmd = std::process::Command::new("qemu-system-x86_64");
    cmd.arg("-serial").arg("stdio");
    if uefi {
        cmd.arg("-bios").arg(ovmf_prebuilt::ovmf_pure_efi());
        cmd.arg("-drive")
            .arg(format!("format=raw,file={uefi_path}"));
    } else {
        cmd.arg("-drive")
            .arg(format!("format=raw,file={bios_path}"));
    }

    // Helps us when we reboot bc of a triple fault
    // cmd.arg("-d").arg("int");
    // cmd.arg("-no-reboot");

    let mut child = cmd.spawn().unwrap();
    child.wait().unwrap();
}
