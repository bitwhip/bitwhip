fn main() {
    let target_os = std::env::var("CARGO_CFG_TARGET_OS").unwrap();

    match target_os.as_str() {
        "macos" | "ios" => {
            println!(
                "cargo:rustc-link-search={}/lib",
                std::env::var("FFMPEG_DIR").expect("FFMPEG_DIR")
            );
        }
        "linux" => {
            println!(
                "cargo:rustc-link-search={}/lib/amd64",
                std::env::var("FFMPEG_DIR").expect("FFMPEG_DIR")
            );
            println!(
                "cargo:rustc-link-search={}/lib",
                std::env::var("FFMPEG_DIR").expect("FFMPEG_DIR")
            );
        }
        "windows" => {
            println!("cargo:rustc-link-arg=/EXPORT:NvOptimusEnablement");
            println!("cargo:rustc-link-arg=/EXPORT:AmdPowerXpressRequestHighPerformance");
            println!(
                "cargo:rustc-link-search={}\\lib\\x64",
                std::env::var("FFMPEG_DIR").expect("FFMPEG_DIR")
            );
            println!(
                "cargo:rustc-link-search={}\\lib",
                std::env::var("FFMPEG_DIR").expect("FFMPEG_DIR")
            );
            // println!(
            //     "cargo:rustc-link-search={}",
            //     std::env::var("OPENSSL_LIBS").expect("OPENSSL_LIBS")
            // );
        }
        tos => panic!("unknown target os {:?}!", tos),
    }
}
