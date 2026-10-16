fn main() {
    #[cfg(windows)]
    winresource::WindowsResource::new()
        .set_icon("src/ui/icon.ico")
        .set("ProductName", "Kirk Companion")
        .set("FileDescription", "Kirk Companion · Native dashboard")
        .set(
            "LegalCopyright",
            "Copyright (c) 2026 Benja (Benjamacs1). MIT License.",
        )
        .compile()
        .expect("Could not embed Windows application resources");
}
