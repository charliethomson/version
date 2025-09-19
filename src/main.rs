use std::path::PathBuf;

use cargo_manifest::Manifest;
use clap::{Parser, ValueEnum};
use colored::*;
use semver::{BuildMetadata, Prerelease, Version};

#[derive(Parser)]
#[clap(version, name = "Workspace Version Upgrade Utility")]
pub struct Args {
    /// If no subcommand is provided, treat the first argument as a version bump
    #[arg(
        value_enum,
        help = "If not provided, configured to read from git, will attempt to infer the bump from the git commit message, else `prepatch`"
    )]
    pub version_bump: Option<VersionBump>,

    // Git-based version inference flag
    #[arg(long, help = "Infer version bump from git commit messages")]
    pub from_git: bool,

    // Expect a workspace instead of a regular project
    #[arg(long, help = "Expect to find a workspace rather than a normal project")]
    pub workspace: bool,

    #[arg(long, value_name = "FILE", help = "Path to commit message file")]
    pub message_file: Option<PathBuf>,

    #[arg(
        long,
        value_name = "FILE",
        help = "Path to manifest file",
        default_value = "Cargo.toml"
    )]
    pub path: PathBuf,

    #[arg(long, help = "Suppress all output except errors")]
    pub quiet: bool,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum VersionBump {
    Prepatch,
    Patch,
    Preminor,
    Minor,
    Major,
    Skip,
}

macro_rules! _vext_def_field {
    ($set:ident, $get:ident, $reset:ident, $inc:ident) => {
        fn $set(self, version: u64) -> Self;
        fn $get(&self) -> u64;
        fn $reset(self) -> Self {
            self.$set(0)
        }
        fn $inc(self) -> Self {
            let v = self.$get();
            self.$set(v + 1)
        }
    };
}
trait VersionExt: Sized {
    _vext_def_field!(set_major, get_major, _reset_major, inc_major);
    _vext_def_field!(set_minor, get_minor, reset_minor, inc_minor);
    _vext_def_field!(set_patch, get_patch, reset_patch, inc_patch);

    fn set_pre(self, version: Option<u64>) -> Self;
    fn get_pre(&self) -> Option<u64>;
    fn reset_pre(self) -> Self {
        self.set_pre(None)
    }
    fn inc_pre(self) -> Self {
        let v = self.get_pre().map(|v| v + 1).unwrap_or(0);
        self.set_pre(Some(v))
    }
}

macro_rules! _vext_impl_field {
    ($field:ident, $set:ident, $get:ident) => {
        fn $set(mut self, version: u64) -> Self {
            self.$field = version;
            self
        }

        fn $get(&self) -> u64 {
            self.$field
        }
    };
}

impl VersionExt for Version {
    _vext_impl_field!(major, set_major, get_major);
    _vext_impl_field!(minor, set_minor, get_minor);
    _vext_impl_field!(patch, set_patch, get_patch);

    fn set_pre(mut self, version: Option<u64>) -> Self {
        if let Some(version) = version {
            self.pre = Prerelease::new(&format!("alpha.{}", version))
                .expect("Prerelease constructor rejected valid prerelease version");
        } else {
            self.pre = Prerelease::EMPTY;
        }

        self
    }

    fn get_pre(&self) -> Option<u64> {
        extract_alpha_version(&self.pre)
    }
}

impl VersionBump {
    fn is_pre(self) -> bool {
        match self {
            VersionBump::Prepatch | VersionBump::Preminor => true,
            VersionBump::Patch | VersionBump::Minor | VersionBump::Major | VersionBump::Skip => {
                false
            }
        }
    }

    fn apply(self, mut version: Version) -> Version {
        version.build = BuildMetadata::EMPTY;

        let has_pre = version.get_pre().is_some();

        if self.is_pre() {
            version = version.inc_pre();
        } else {
            version = version.reset_pre();
        }

        match self {
            VersionBump::Patch | VersionBump::Prepatch if !has_pre => version.inc_patch(),
            VersionBump::Patch => version.reset_pre(),

            VersionBump::Minor | VersionBump::Preminor if !has_pre => {
                version.inc_minor().reset_patch()
            }
            VersionBump::Minor => version.reset_patch(),

            VersionBump::Major => version.inc_major().reset_minor().reset_patch().reset_pre(),

            _ => version,
        }
    }

    fn description(&self) -> &'static str {
        match self {
            VersionBump::Major => "major release",
            VersionBump::Minor => "minor release",
            VersionBump::Patch => "patch release",
            VersionBump::Preminor => "pre-minor alpha",
            VersionBump::Prepatch => "pre-patch alpha",
            VersionBump::Skip => "skip version bump",
        }
    }

    fn emoji(&self) -> &'static str {
        match self {
            VersionBump::Major => "🚀",
            VersionBump::Minor => "✨",
            VersionBump::Patch => "🔧",
            VersionBump::Preminor => "🧪",
            VersionBump::Prepatch => "🔬",
            VersionBump::Skip => "⏭️",
        }
    }

    fn color(&self) -> Color {
        match self {
            VersionBump::Major => Color::Red,
            VersionBump::Minor => Color::Blue,
            VersionBump::Patch => Color::Green,
            VersionBump::Preminor | VersionBump::Prepatch => Color::Yellow,
            VersionBump::Skip => Color::White,
        }
    }
}

fn extract_version(args: &Args, manifest: &Manifest) -> anyhow::Result<Version> {
    let version_field = if args.workspace {
        manifest
            .workspace
            .as_ref()
            .ok_or(anyhow::anyhow!("Expected to find a workspace"))?
            .package
            .as_ref()
            .ok_or(anyhow::anyhow!(
                "Expected to find a package section in the workspace"
            ))?
            .version
            .as_ref()
            .ok_or(anyhow::anyhow!(
                "Expected to find a package version in the package section"
            ))?
            .clone()
    } else {
        manifest
            .package
            .as_ref()
            .ok_or(anyhow::anyhow!(
                "Expected to find a package section in the manifest"
            ))?
            .version
            .as_ref()
            .ok_or(anyhow::anyhow!(
                "Expected to find a package version in the package section"
            ))?
            .clone()
            .as_local()
            .ok_or(anyhow::anyhow!(
                "The package version is inherited from a workspace (use --workspace)"
            ))?
    };

    Ok(semver::Version::parse(&version_field)?)
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();

    let manifest = Manifest::from_path(&args.path)?;

    let version = extract_version(&args, &manifest)?;
    let old_version = version.clone().to_string();

    let version_bump = args
        .version_bump
        .or(infer_version_bump(&args))
        .unwrap_or(VersionBump::Prepatch);

    if matches!(version_bump, VersionBump::Skip) {
        if !args.quiet {
            println!(
                "{} {}",
                version_bump.emoji(),
                version_bump.description().color(version_bump.color())
            );
        }
        return Ok(());
    }

    let new_version = version_bump.apply(version).to_string();

    if !args.quiet {
        println!(
            "{} {} {} {} {} {}",
            version_bump.emoji(),
            "Version bump:".bold().blue(),
            old_version.cyan(),
            "→".bright_white(),
            new_version.bright_green().bold(),
            format!("({})", version_bump.description()).color(version_bump.color())
        );
    }

    let field = if args.workspace {
        "package.version"
    } else {
        "version"
    };

    let file_content = std::fs::read_to_string(&args.path)?.replace(
        &format!("{field} = \"{old_version}\""),
        &format!("{field} = \"{new_version}\""),
    );
    std::fs::write(&args.path, file_content)?;

    if !args.quiet {
        println!(
            "{} Updated {}",
            "✓".green().bold(),
            args.path.display().to_string().bold()
        );
    }

    Ok(())
}

fn infer_version_bump(args: &Args) -> Option<VersionBump> {
    if !args.from_git {
        return None;
    }
    let message_file = args.message_file.as_ref()?;
    let commit_message = std::fs::read_to_string(message_file).ok()?.to_lowercase();

    let map = vec![
        ("[major]", VersionBump::Major),
        ("[minor]", VersionBump::Minor),
        ("[patch]", VersionBump::Patch),
        ("[preminor]", VersionBump::Preminor),
        ("[prepatch]", VersionBump::Prepatch),
        ("[no-version]", VersionBump::Skip),
    ];

    for (pattern, bump) in &map {
        if commit_message.contains(pattern) {
            return Some(*bump);
        }
    }

    None
}

/// Extract the numeric part from an "-alpha.X" prerelease identifier
/// Returns Some(X) if the prerelease is in the format "alpha.X", None otherwise
fn extract_alpha_version(prerelease: &Prerelease) -> Option<u64> {
    let pre_str = prerelease.as_str();
    if let Some(suffix) = pre_str.strip_prefix("alpha.") {
        suffix.parse::<u64>().ok()
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use semver::Version;

    macro_rules! do_test {
        ($name:ident, $current:literal, $bump:ident, $expected:literal) => {
            #[test]
            fn $name() {
                let version = Version::parse($current).unwrap();
                let result = VersionBump::$bump.apply(version);
                assert_eq!(result.to_string(), $expected);
            }
        };
    }

    do_test!(major_bump, "1.2.3", Major, "2.0.0");
    do_test!(minor_bump, "1.2.3", Minor, "1.3.0");
    do_test!(patch_bump, "1.2.3", Patch, "1.2.4");

    do_test!(preminor_first_time, "1.2.3", Preminor, "1.3.0-alpha.0");
    do_test!(
        preminor_increment_existing,
        "1.3.0-alpha.2",
        Preminor,
        "1.3.0-alpha.3"
    );
    do_test!(prepatch_first_time, "1.2.3", Prepatch, "1.2.4-alpha.0");
    do_test!(
        prepatch_increment_existing,
        "1.2.4-alpha.1",
        Prepatch,
        "1.2.4-alpha.2"
    );

    do_test!(patch_clears_pre, "1.2.3-alpha.0", Patch, "1.2.3");
    do_test!(minor_clears_pre, "1.2.3-alpha.0", Minor, "1.2.0");
    do_test!(major_clears_pre, "1.2.3-alpha.0", Major, "2.0.0");

    #[test]
    fn test_realistic() {
        macro_rules! apply_and_assert {
            ($v:ident, $bump:ident, $expected:literal) => {
                println!("{}", $v);
                let $v = VersionBump::$bump.apply($v);
                println!("{}", $v);
                assert_eq!($v.to_string(), $expected);
            };
        }

        let version = Version::parse("0.1.0").unwrap();
        apply_and_assert!(version, Prepatch, "0.1.1-alpha.0");
        apply_and_assert!(version, Prepatch, "0.1.1-alpha.1");
        apply_and_assert!(version, Prepatch, "0.1.1-alpha.2");
        apply_and_assert!(version, Prepatch, "0.1.1-alpha.3");
        apply_and_assert!(version, Prepatch, "0.1.1-alpha.4");
        apply_and_assert!(version, Patch, "0.1.1");
        apply_and_assert!(version, Preminor, "0.2.0-alpha.0");
        apply_and_assert!(version, Minor, "0.2.0");
        apply_and_assert!(version, Major, "1.0.0");
    }

    #[test]
    fn test_non_alpha_prerelease_treated_as_no_prerelease() {
        let version = Version::parse("1.2.3-beta.1").unwrap();
        let result = VersionBump::Preminor.apply(version);
        assert_eq!(result.to_string(), "1.3.0-alpha.0");
    }

    #[test]
    fn test_extract_alpha_version() {
        let pre1 = semver::Prerelease::new("alpha.0").unwrap();
        let pre2 = semver::Prerelease::new("alpha.42").unwrap();
        let pre3 = semver::Prerelease::new("beta.1").unwrap();
        let pre4 = semver::Prerelease::new("alpha").unwrap();

        assert_eq!(extract_alpha_version(&pre1), Some(0));
        assert_eq!(extract_alpha_version(&pre2), Some(42));
        assert_eq!(extract_alpha_version(&pre3), None);
        assert_eq!(extract_alpha_version(&pre4), None);
    }
}
