fn main() {
    println!("cargo:rerun-if-env-changed=AX_CONFIG_PATH");
    if let Ok(config_path) = std::env::var("AX_CONFIG_PATH") {
        println!("cargo:rerun-if-changed={config_path}");

        // Check whether the `package` field in the `AXCONFIG_PATH` file matches the current package name.
        let output = std::process::Command::new("axconfig-gen")
            .arg(&config_path)
            .arg("-r")
            .arg("package")
            .output()
            .expect("Failed to execute axconfig-gen");

        if !output.status.success() {
            panic!(
                "Failed to run axconfig-gen: {}",
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let package_name = format!(
            "axplat-{}",
            String::from_utf8(output.stdout)
                .expect("Failed to parse axconfig-gen output")
                .trim()
                .trim_matches('"')
        );
        if package_name != env!("CARGO_PKG_NAME") {
            panic!(
                "The current package {} does not match the package field {} in the configuration file {}",
                env!("CARGO_PKG_NAME"),
                package_name,
                config_path
            );
        }
    }
}
