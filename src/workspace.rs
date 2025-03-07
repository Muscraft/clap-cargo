//! Cargo flags for selecting crates in a workspace.

use std::collections;

#[derive(Default, Clone, Debug, PartialEq, Eq, clap::Args)]
#[non_exhaustive]
pub struct Workspace {
    #[clap(short, long, value_name = "SPEC")]
    /// Package to process (see `cargo help pkgid`)
    pub package: Vec<String>,
    #[clap(long)]
    /// Process all packages in the workspace
    pub workspace: bool,
    #[clap(long, hide_short_help(true), hide_long_help(true))]
    /// Process all packages in the workspace
    pub all: bool,
    #[clap(long, value_name = "SPEC")]
    /// Exclude packages from being processed
    pub exclude: Vec<String>,
}

#[cfg(feature = "cargo_metadata")]
impl Workspace {
    /// Partition workspace members into those selected and those excluded.
    ///
    /// Notes:
    /// - Requires the features `cargo_metadata`.
    /// - Requires not calling `MetadataCommand::no_deps`
    pub fn partition_packages<'m>(
        &self,
        meta: &'m cargo_metadata::Metadata,
    ) -> (
        Vec<&'m cargo_metadata::Package>,
        Vec<&'m cargo_metadata::Package>,
    ) {
        let selection =
            Packages::from_flags(self.workspace || self.all, &self.exclude, &self.package);
        let workspace_members: collections::HashSet<_> = meta.workspace_members.iter().collect();
        let base_ids: collections::HashSet<_> = match selection {
            Packages::Default => {
                // Deviating from cargo because Metadata doesn't have default members
                let resolve = meta.resolve.as_ref().expect("no-deps is unsupported");
                match &resolve.root {
                    Some(root) => {
                        let mut base_ids = collections::HashSet::new();
                        base_ids.insert(root);
                        base_ids
                    }
                    None => workspace_members,
                }
            }
            Packages::All => workspace_members,
            Packages::OptOut(_) => workspace_members, // Deviating from cargo by only checking workspace members
            Packages::Packages(patterns) => {
                meta.packages
                    .iter()
                    // Deviating from cargo by not supporting patterns
                    // Deviating from cargo by only checking workspace members
                    .filter(|p| workspace_members.contains(&p.id) && patterns.contains(&p.name))
                    .map(|p| &p.id)
                    .collect()
            }
        };

        meta.packages
            .iter()
            // Deviating from cargo by not supporting patterns
            .partition(|p| base_ids.contains(&p.id) && !self.exclude.contains(&p.name))
    }
}

// See cargo's src/cargo/ops/cargo_compile.rs
#[derive(Clone, PartialEq, Eq, Debug)]
#[cfg(feature = "cargo_metadata")]
#[allow(clippy::enum_variant_names)]
enum Packages<'p> {
    Default,
    All,
    OptOut(&'p [String]),
    Packages(&'p [String]),
}

#[cfg(feature = "cargo_metadata")]
impl<'p> Packages<'p> {
    pub fn from_flags(all: bool, exclude: &'p [String], package: &'p [String]) -> Self {
        match (all, exclude.len(), package.len()) {
            (false, 0, 0) => Packages::Default,
            (false, 0, _) => Packages::Packages(package),
            (false, _, 0) => Packages::OptOut(exclude), // Deviating from cargo because we don't do error handling
            (false, _, _) => Packages::Packages(package), // Deviating from cargo because we don't do error handling
            (true, 0, _) => Packages::All,
            (true, _, _) => Packages::OptOut(exclude),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    use clap::StructOpt;

    #[test]
    fn verify_app() {
        #[derive(Debug, clap::StructOpt)]
        struct Cli {
            #[clap(flatten)]
            workspace: Workspace,
        }

        use clap::CommandFactory;
        Cli::command().debug_assert()
    }

    #[test]
    fn parse_multiple_occurrences() {
        #[derive(PartialEq, Eq, Debug, StructOpt)]
        struct Args {
            positional: Option<String>,
            #[clap(flatten)]
            workspace: Workspace,
        }

        assert_eq!(
            Args {
                positional: None,
                workspace: Workspace {
                    package: vec![],
                    workspace: false,
                    all: false,
                    exclude: vec![],
                }
            },
            Args::parse_from(&["test"])
        );
        assert_eq!(
            Args {
                positional: Some("baz".to_owned()),
                workspace: Workspace {
                    package: vec!["foo".to_owned(), "bar".to_owned()],
                    workspace: false,
                    all: false,
                    exclude: vec![],
                }
            },
            Args::parse_from(&["test", "--package", "foo", "--package", "bar", "baz"])
        );
        assert_eq!(
            Args {
                positional: Some("baz".to_owned()),
                workspace: Workspace {
                    package: vec![],
                    workspace: false,
                    all: false,
                    exclude: vec!["foo".to_owned(), "bar".to_owned()],
                }
            },
            Args::parse_from(&["test", "--exclude", "foo", "--exclude", "bar", "baz"])
        );
    }

    #[cfg(feature = "cargo_metadata")]
    #[cfg(test)]
    mod partition_default {
        use super::*;

        #[test]
        fn single_crate() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/simple/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn mixed_ws_root() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/mixed_ws/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }

        #[test]
        fn mixed_ws_leaf() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/mixed_ws/c/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }

        #[test]
        fn pure_ws_root() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/pure_ws/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 3);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn pure_ws_leaf() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/pure_ws/c/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }
    }

    #[cfg(feature = "cargo_metadata")]
    #[cfg(test)]
    mod partition_all {
        use super::*;

        #[test]
        fn single_crate() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/simple/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                all: true,
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn mixed_ws_root() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/mixed_ws/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                all: true,
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 3);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn mixed_ws_leaf() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/mixed_ws/c/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                all: true,
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 3);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn pure_ws_root() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/pure_ws/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                all: true,
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 3);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn pure_ws_leaf() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/pure_ws/c/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                all: true,
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 3);
            assert_eq!(excluded.len(), 0);
        }
    }

    #[cfg(feature = "cargo_metadata")]
    #[cfg(test)]
    mod partition_package {
        use super::*;

        #[test]
        fn single_crate() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/simple/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                package: vec!["simple".to_owned()],
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 0);
        }

        #[test]
        fn mixed_ws_root() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/mixed_ws/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                package: vec!["a".to_owned()],
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }

        #[test]
        fn mixed_ws_leaf() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/mixed_ws/c/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                package: vec!["a".to_owned()],
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }

        #[test]
        fn pure_ws_root() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/pure_ws/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                package: vec!["a".to_owned()],
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }

        #[test]
        fn pure_ws_leaf() {
            let mut metadata = cargo_metadata::MetadataCommand::new();
            metadata.manifest_path("tests/fixtures/pure_ws/c/Cargo.toml");
            let metadata = metadata.exec().unwrap();

            let workspace = Workspace {
                package: vec!["a".to_owned()],
                ..Default::default()
            };
            let (included, excluded) = workspace.partition_packages(&metadata);
            assert_eq!(included.len(), 1);
            assert_eq!(excluded.len(), 2);
        }
    }
}
