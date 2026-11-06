fn main() {
    slint_build::compile("ui/app.slint").expect("Failed to compile Slint UI");

    #[cfg(windows)]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("graphics/app_icon.ico");
        if let Err(e) = res.compile() {
            eprintln!("Warning: Failed to compile Windows resource icon: {}", e);
        }
    }
}
