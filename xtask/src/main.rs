use std::collections::HashSet;

fn discover_plugin_packages(offline: bool) -> nih_plug_xtask::Result<Vec<String>> {
    nih_plug_xtask::chdir_workspace_root()?;
    let mut cmd = cargo_metadata::MetadataCommand::new();
    cmd.manifest_path("./Cargo.toml").no_deps();
    if offline {
        cmd.other_options(["--offline".to_string()]);
    }
    let metadata = cmd.exec()?;
    let workspace_ids: HashSet<_> = metadata.workspace_members.iter().collect();

    let mut plugins: Vec<String> = metadata
        .packages
        .iter()
        .filter(|pkg| workspace_ids.contains(&pkg.id))
        .filter(|pkg| {
            pkg.targets
                .iter()
                .any(|t| t.crate_types.iter().any(|ct| ct == "cdylib"))
        })
        .map(|pkg| pkg.name.clone())
        .collect();
    plugins.sort();
    Ok(plugins)
}

fn main() -> nih_plug_xtask::Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if let Some(first) = args.first() {
        if first == "bundle-all" || first == "bundle-all-universal" {
            let rest = &args[1..];
            let offline = rest.iter().any(|a| a == "--offline");
            let plugins = discover_plugin_packages(offline)?;
            if plugins.is_empty() {
                eprintln!("No cdylib plugin packages found in workspace.");
                return Ok(());
            }
            let subcmd = if first == "bundle-all-universal" {
                "bundle-universal"
            } else {
                "bundle"
            };
            let mut forwarded = Vec::with_capacity(1 + plugins.len() * 2 + rest.len());
            forwarded.push(subcmd.to_string());
            for pkg in &plugins {
                forwarded.push("-p".to_string());
                forwarded.push(pkg.clone());
            }
            forwarded.extend(rest.iter().cloned());
            eprintln!("Bundling workspace plugins: {}", plugins.join(", "));
            return nih_plug_xtask::main_with_args("cargo xtask", forwarded);
        }
    }

    nih_plug_xtask::main()
}
