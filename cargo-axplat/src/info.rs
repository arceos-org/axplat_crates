use camino::{Utf8Path, Utf8PathBuf};
use cargo_metadata::{CargoOpt, MetadataCommand, Package};
use clap::Parser;
use toml_edit::DocumentMut;

/// Display information about a platform package
#[derive(Parser, Debug)]
#[command(long_about = "Display information about a platform package")]
pub struct CommandInfo {
    /// Package to inspect
    ///
    /// Requires that the package has been added to dependencies,
    /// e.g, by using `cargo axplat add <PLATFORM>`.
    #[arg(required = true, value_name = "PLATFORM")]
    package: String,

    /// The drectory to run `cargo axplat`.
    #[arg(short = 'C')]
    directory: Option<String>,

    /// Path to Cargo.toml
    #[arg(
        long = "manifest-path",
        value_name = "PATH",
        help_heading = "Manifest Options"
    )]
    manifest_path: Option<String>,

    /// Display the platform name
    #[arg(short = 'p', long = "platform")]
    plat: bool,

    /// Display the architecture of the platform
    #[arg(short = 'a', long = "arch")]
    arch: bool,

    /// Display the version of the platform package
    #[arg(short = 'v', long = "version")]
    version: bool,

    /// Display the source of the platform package
    #[arg(short = 's', long = "source")]
    source: bool,

    /// Display the path to the platform configuration file
    #[arg(short = 'c', long = "config-path")]
    config_path: bool,
}

#[derive(Debug, thiserror::Error)]
enum PlatformInfoErr {
    #[error("{0}")]
    Metadata(#[from] cargo_metadata::Error),

    #[error("platform package `{0}` not found in dependencies")]
    PackageNotFound(String),

    #[error("configuration file not found at `{0}`")]
    NoConfig(String),

    #[error("error reading configuration file `{1}`: {0}")]
    ConfigIo(#[source] std::io::Error, String),

    #[error("invalid configuration file `{0}`")]
    InvalidConfig(String),

    #[error("invalid manifest path `{0}`")]
    InvalidManifestPath(String),
}

struct PlatformInfo {
    platform: String,
    arch: String,
    version: String,
    source: String,
    config_path: Utf8PathBuf,
}

impl PlatformInfo {
    fn new(package: &Package) -> Result<Self, PlatformInfoErr> {
        let version = package.version.to_string();
        let source = if let Some(src) = &package.source {
            src.to_string()
        } else {
            package.id.to_string()
        };

        let manifest_path = Utf8PathBuf::from_path_buf(package.manifest_path.clone().into())
            .map_err(|p| PlatformInfoErr::InvalidManifestPath(p.to_string_lossy().into_owned()))?;
        let root_dir = manifest_path
            .parent()
            .ok_or_else(|| PlatformInfoErr::InvalidManifestPath(manifest_path.to_string()))?;
        let config_path = root_dir.join("axconfig.toml");
        let (platform, arch) = parse_config(&config_path)?;
        Ok(Self {
            platform,
            arch,
            version,
            source,
            config_path,
        })
    }

    fn from(args: &CommandInfo) -> Result<Self, PlatformInfoErr> {
        let mut metadata_handler = MetadataCommand::new()
            .features(CargoOpt::AllFeatures)
            .verbose(true)
            .clone();

        if let Some(dir) = &args.directory {
            metadata_handler.current_dir(dir);
        }
        if let Some(manifest_path) = &args.manifest_path {
            metadata_handler.manifest_path(manifest_path);
        }
        let metadata = metadata_handler.exec().map_err(PlatformInfoErr::Metadata)?;
        for p in metadata.packages {
            if p.name.as_str() == args.package {
                return Self::new(&p);
            }
        }
        Err(PlatformInfoErr::PackageNotFound(args.package.clone()))
    }

    fn display(&self, args: &CommandInfo) {
        if args.plat {
            println!("{}", self.platform);
        }
        if args.arch {
            println!("{}", self.arch);
        }
        if args.version {
            println!("{}", self.version);
        }
        if args.source {
            println!("{}", self.source);
        }
        if args.config_path {
            println!("{}", self.config_path);
        }
    }

    fn display_all(&self) {
        println!("platform: {}", self.platform);
        println!("arch: {}", self.arch);
        println!("version: {}", self.version);
        println!("source: {}", self.source);
        println!("config_path: {}", self.config_path);
    }
}

fn parse_config(config_path: &Utf8Path) -> Result<(String, String), PlatformInfoErr> {
    let toml = std::fs::read_to_string(config_path.as_std_path()).map_err(|err| {
        match err.kind() {
            std::io::ErrorKind::NotFound => PlatformInfoErr::NoConfig(config_path.to_string()),
            _ => PlatformInfoErr::ConfigIo(err, config_path.to_string()),
        }
    })?;
    (|| {
        let config = toml.parse::<DocumentMut>().ok()?;
        let plat_name = config["platform"].as_str()?.to_string();
        let arch = config["arch"].as_str()?.to_string();
        Some((plat_name, arch))
    })()
    .ok_or_else(|| PlatformInfoErr::InvalidConfig(config_path.to_string()))
}

pub fn platform_info(args: CommandInfo) {
    match PlatformInfo::from(&args) {
        Ok(info) => {
            if args.plat || args.arch || args.version || args.source || args.config_path {
                info.display(&args);
            } else {
                info.display_all();
            }
        }
        Err(err) => {
            eprintln!("error: {err}");
            std::process::exit(1);
        }
    }
}
