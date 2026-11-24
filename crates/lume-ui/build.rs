fn main() {
    println!("cargo:rerun-if-changed=assets/icons/lume-music.ico");

    #[cfg(target_os = "windows")]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icons/lume-music.ico");
        res.set("ProductName", "Lume Music");
        res.set("FileDescription", "Lume Music — Native Rust Desktop Music Player");
        res.set("OriginalFilename", "lume-music.exe");
        res.compile().expect("failed to compile Windows icon resource");
    }
}
