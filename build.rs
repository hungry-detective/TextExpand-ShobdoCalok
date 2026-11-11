//! Embeds the app icon (resource id 1 — also used by the tray) and the
//! application manifest into ShobdoCalok.exe.

fn main() {
    println!("cargo:rerun-if-changed=assets/app-icon.ico");
    println!("cargo:rerun-if-changed=assets/app.manifest");
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=client_secret.json");
    // Embed the Google OAuth client if the developer placed it in the project root.
    {
        let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
        let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
        let secret = std::fs::read_to_string(manifest_dir.join("client_secret.json")).unwrap_or_default();
        std::fs::write(out_dir.join("embedded_client_secret.json"), secret).expect("write embedded secret");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    if std::env::var("CARGO_CFG_TARGET_ENV").as_deref() == Ok("gnu") {
        gnu_windres();
    } else {
        let mut res = winresource::WindowsResource::new();
        res.set_icon_with_id("assets/app-icon.ico", "1");
        res.set_manifest_file("assets/app.manifest");
        res.set("ProductName", "Shobdo Calok");
        res.set("FileDescription", "Shobdo Calok");
        if let Err(e) = res.compile() {
            println!("cargo:warning=failed to compile Windows resources: {e}");
        }
    }
}

/// windres cannot cope with spaces in the include path, so compile the .rc in
/// a space-free scratch folder and link the object file directly.
fn gnu_windres() {
    let manifest_dir = std::path::PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").unwrap());
    let work = std::env::temp_dir().join(format!("shobdo_calok_res_{}", std::process::id()));
    let _ = std::fs::create_dir_all(&work);
    let _ = std::fs::copy(manifest_dir.join("assets/app-icon.ico"), work.join("app.ico"));
    let _ = std::fs::copy(manifest_dir.join("assets/app.manifest"), work.join("app.manifest"));
    let ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_else(|_| "1.0.0".into());
    let parts: Vec<&str> = ver.split('.').collect();
    let v = |i: usize| parts.get(i).copied().unwrap_or("0");
    let rc = format!(
        r#"1 ICON "app.ico"
1 24 "app.manifest"
1 VERSIONINFO
FILEVERSION {a},{b},{c},0
PRODUCTVERSION {a},{b},{c},0
BEGIN
  BLOCK "StringFileInfo"
  BEGIN
    BLOCK "040904b0"
    BEGIN
      VALUE "ProductName", "Shobdo Calok"
      VALUE "FileDescription", "Shobdo Calok"
      VALUE "FileVersion", "{ver}"
      VALUE "ProductVersion", "{ver}"
      VALUE "OriginalFilename", "ShobdoCalok.exe"
    END
  END
  BLOCK "VarFileInfo"
  BEGIN
    VALUE "Translation", 0x409, 1200
  END
END
"#,
        a = v(0),
        b = v(1),
        c = v(2),
    );
    std::fs::write(work.join("app.rc"), rc).expect("write rc");
    let status = std::process::Command::new("windres")
        .current_dir(&work)
        .args(["app.rc", "-O", "coff", "-o", "app_res.o"])
        .status();
    match status {
        Ok(s) if s.success() => {
            let out_dir = std::path::PathBuf::from(std::env::var("OUT_DIR").unwrap());
            let obj = out_dir.join("app_res.o");
            std::fs::copy(work.join("app_res.o"), &obj).expect("copy res");
            println!("cargo:rustc-link-arg-bins={}", obj.display());
        }
        other => println!("cargo:warning=windres failed: {other:?}"),
    }
    let _ = std::fs::remove_dir_all(&work);
}
