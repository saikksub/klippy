fn main() {
    println!("cargo:rerun-if-changed=assets/Klippy.ico");
    // The exe's icon and version details. This needs the Windows SDK's
    // resource compiler, so it's skipped when checking the crate from
    // another OS.
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") && cfg!(windows) {
        let mut resource = winresource::WindowsResource::new();
        resource
            .set_icon("assets/Klippy.ico")
            .set("ProductName", "Klippy")
            .set("FileDescription", "Klippy")
            .set("LegalCopyright", "MIT licence");
        if let Err(error) = resource.compile() {
            println!("cargo:warning=couldn't embed the icon: {error}");
        }
    }
}
