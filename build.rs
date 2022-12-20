use std::collections::HashMap;
use std::env;
use std::fs::File;
use std::path::PathBuf;
use std::process::Command;

fn main() {
    let qt_include_path = env::var("DEP_QT_INCLUDE_PATH").unwrap();
    let qt_library_path = env::var("DEP_QT_LIBRARY_PATH").unwrap();
    let qt_version = env::var("DEP_QT_VERSION").unwrap();

    let mut config = cpp_build::Config::new();

    for f in std::env::var("DEP_QT_COMPILE_FLAGS")
        .unwrap()
        .split_terminator(";")
    {
        config.flag(f);
    }

    let mut public_include = |name| {
        if cfg!(target_os = "macos") {
            config.include(format!("{}/{}.framework/Headers/", qt_library_path, name));
        }
        config.include(format!("{}/{}", qt_include_path, name));
    };
    public_include("QtCore");
    public_include("QtGui");
    public_include("QtQuick");
    public_include("QtQml");

    let mut private_include = |name| {
        if cfg!(target_os = "macos") {
            config.include(format!(
                "{}/{}.framework/Headers/{}",
                qt_library_path, name, qt_version
            ));
            config.include(format!(
                "{}/{}.framework/Headers/{}/{}",
                qt_library_path, name, qt_version, name
            ));
        }
        config
            .include(format!("{}/{}/{}", qt_include_path, name, qt_version))
            .include(format!(
                "{}/{}/{}/{}",
                qt_include_path, name, qt_version, name
            ));
    };
    private_include("QtCore");
    private_include("QtGui");
    private_include("QtQuick");
    private_include("QtQml");

    #[cfg(feature = "mdk-nightly")]
    let nightly = "nightly/";
    #[cfg(not(feature = "mdk-nightly"))]
    let nightly = "";

    let sdk: HashMap<&str, (String, &str, &str, &str)> = vec![
        ("windows",  (format!("https://master.dl.sourceforge.net/project/mdk-sdk/{}mdk-sdk-windows-desktop-vs2022.7z?viasf=1", nightly), "lib/x64/",           "mdk.lib",    "include/")),
        ("linux",    (format!("https://master.dl.sourceforge.net/project/mdk-sdk/{}mdk-sdk-linux.tar.xz?viasf=1", nightly),              "lib/amd64/",         "libmdk.so",  "include/")),
        ("macos",    (format!("https://master.dl.sourceforge.net/project/mdk-sdk/{}mdk-sdk-macOS.tar.xz?viasf=1", nightly),              "lib/mdk.framework/", "mdk",        "include/")),
        ("android",  (format!("https://master.dl.sourceforge.net/project/mdk-sdk/{}mdk-sdk-android.7z?viasf=1", nightly),                "lib/arm64-v8a/",     "libmdk.so",  "include/")),
        ("ios",      (format!("https://master.dl.sourceforge.net/project/mdk-sdk/{}mdk-sdk-iOS.tar.xz?viasf=1", nightly),                "lib/mdk.framework/", "mdk",        "include/")),
    ].into_iter().collect();

    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let entry = &sdk[target_os.as_str()];

    if let Ok(path) = download_and_extract(&entry.0, &format!("{}{}", entry.1, entry.2)) {
        if target_os == "macos" || target_os == "ios" {
            println!(
                "cargo:rustc-link-search=framework={}{}",
                path.display(),
                "lib/"
            );
            println!("cargo:rustc-link-lib=framework=mdk");
            config.flag_if_supported("-fobjc-arc");
            config.flag("-x").flag("objective-c++");
            let frameworks_dir =
                PathBuf::from(env::var("OUT_DIR").unwrap()).join("../../../../Frameworks");
            if !frameworks_dir.exists() {
                std::fs::create_dir(&frameworks_dir).unwrap();
            }
            std::fs::copy(path.join("lib/mdk.framework"), &frameworks_dir).unwrap();
        } else {
            println!("cargo:rustc-link-search={}{}", path.display(), entry.1);
            println!("cargo:rustc-link-lib=mdk");
        }
        if target_os == "windows" {
            std::fs::copy(
                path.join("bin/x64/mdk.dll"),
                format!("{}/../../../mdk.dll", env::var("OUT_DIR").unwrap()),
            )
            .unwrap();
            std::fs::copy(
                path.join("bin/x64/ffmpeg-5.dll"),
                format!("{}/../../../ffmpeg-5.dll", env::var("OUT_DIR").unwrap()),
            )
            .unwrap();
            let _ = std::fs::copy(
                path.join("bin/x64/mdk-braw.dll"),
                format!("{}/../../../mdk-braw.dll", env::var("OUT_DIR").unwrap()),
            );
        }
        if target_os == "android" {
            std::fs::copy(
                path.join("lib/arm64-v8a/libmdk.so"),
                format!("{}/../../../libmdk.so", env::var("OUT_DIR").unwrap()),
            )
            .unwrap();
            std::fs::copy(
                path.join("lib/arm64-v8a/libffmpeg.so"),
                format!("{}/../../../libffmpeg.so", env::var("OUT_DIR").unwrap()),
            )
            .unwrap();
            // std::fs::copy(format!("{}/lib/arm64-v8a/libqtav-mediacodec.so", path), format!("{}/../../../libqtav-mediacodec.so", env::var("OUT_DIR").unwrap())).unwrap();
        }
        if target_os == "linux" {
            std::fs::copy(
                path.join("lib/amd64/libffmpeg.so.5"),
                format!("{}/../../../libffmpeg.so.5", env::var("OUT_DIR").unwrap()),
            )
            .unwrap();
            std::fs::copy(
                path.join("lib/amd64/libmdk.so.0"),
                format!("{}/../../../libmdk.so.0", env::var("OUT_DIR").unwrap()),
            )
            .unwrap();
            let _ = std::fs::copy(
                path.join("lib/amd64/libmdk-braw.so"),
                format!("{}/../../../libmdk-braw.so", env::var("OUT_DIR").unwrap()),
            );
        }
        config.include(path.join(entry.3));
    } else {
        panic!("Unable to download or extract mdk-sdk. Please make sure you have 7z in PATH or download mdk manually from https://sourceforge.net/projects/mdk-sdk/ and extract to {}", env::var("OUT_DIR").unwrap());
    }

    let vulkan_sdk = env::var("VULKAN_SDK");
    if let Ok(sdk) = vulkan_sdk {
        if !sdk.is_empty() {
            config.include(format!("{}/Include", sdk));
            config.include(format!("{}/include", sdk));
        }
    }

    config.include(&qt_include_path).build("src/lib.rs");
}

fn download_and_extract(url: &str, check: &str) -> Result<PathBuf, std::io::Error> {
    let mdk_sdk_root =
        PathBuf::from(&env::var("MDK_SDK_ROOT").unwrap_or(env::var("OUT_DIR").unwrap()));

    if mdk_sdk_root.join(check).exists() {
        return Ok(mdk_sdk_root);
    }

    let ext = if url.contains(".tar.xz") {
        ".tar.xz"
    } else {
        ".7z"
    };
    let archive_path = mdk_sdk_root.join(format!("mdk-sdk{}", ext));
    {
        let mut reader = ureq::get(url)
            .call()
            .map_err(|_| std::io::ErrorKind::Other)?
            .into_reader();
        let mut file = File::create(archive_path.clone())?;
        std::io::copy(&mut reader, &mut file)?;
    }
    Command::new("7z")
        .current_dir(&mdk_sdk_root)
        .args(&["x", "-y", archive_path.to_str().unwrap()])
        .status()?;
    std::fs::remove_file(archive_path)?;

    if ext == ".tar.xz" {
        let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
        if target_os == "macos" || target_os == "ios" || target_os == "linux" {
            Command::new("tar")
                .current_dir(&mdk_sdk_root)
                .args(&["-xf", "mdk-sdk.tar"])
                .status()?;
        } else {
            Command::new("7z")
                .current_dir(&mdk_sdk_root)
                .args(&["x", "-y", "mdk-sdk.tar"])
                .status()?;
        }
        std::fs::remove_file(mdk_sdk_root.join("mdk-sdk.tar"))?;
    }

    Ok(mdk_sdk_root)
}
