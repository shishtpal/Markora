fn main() {
    #[cfg(windows)]
    if std::env::var_os("CARGO_CFG_WINDOWS").is_some() {
        println!("cargo:rerun-if-changed=assets/markora.ico");
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/markora.ico");
        if let Err(err) = res.compile() {
            panic!("failed to compile Windows resources: {err}");
        }
    }
}
