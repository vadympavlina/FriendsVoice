//! Embeds the application icon and version info into friends-voice.exe so the
//! file shows the mascot in Explorer, the taskbar and the Start menu.
fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");
    #[cfg(windows)]
    {
        let mut resource = winresource::WindowsResource::new();
        resource.set_icon("assets/icon.ico");
        resource.set("ProductName", "Friends Voice");
        resource.set("FileDescription", "Friends Voice");
        resource.set("CompanyName", "Friends Voice");
        // A missing resource compiler must never break the build: the app
        // still runs, it just keeps the default file icon.
        if let Err(error) = resource.compile() {
            println!("cargo:warning=could not embed the exe icon: {error}");
        }
    }
}
