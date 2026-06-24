// build.rs — embed a Windows VERSIONINFO resource so hodos-wallet.exe carries
// publisher/version metadata (Marston Enterprises). A metadata-less PE inside a
// self-built Chromium app is a textbook Defender/SmartScreen heuristic trigger.
// (Goal-2 signing hygiene.) No-op on non-Windows.
fn main() {
    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set("CompanyName", "Marston Enterprises");
        res.set("ProductName", "Hodos Wallet");
        res.set("FileDescription", "Hodos Browser wallet backend");
        res.set("OriginalFilename", "hodos-wallet.exe");
        res.set("LegalCopyright", "Copyright (C) Marston Enterprises");
        // FileVersion / ProductVersion default to CARGO_PKG_VERSION.
        if let Err(e) = res.compile() {
            // Don't fail the build if the resource compiler is unavailable (e.g. a dev
            // box without the Windows SDK rc.exe). Release/CI builds have it.
            println!("cargo:warning=winresource: could not embed VERSIONINFO: {e}");
        }
    }
}
