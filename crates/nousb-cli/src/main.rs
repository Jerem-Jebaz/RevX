use nousb_core::{
    built_in_profiles, create_recovery_pack, embedded_profiles, execute_plan_safe,
    export_diagnostic_bundle, list_saved_execution_reports, load_execution_report_json,
    plan_operation, plan_operation_for_inventory, rollback_simulated_report, save_execution_report,
    verify_sha256_with_system_tool, write_journal_entry, FakePlatformAdapter, HostPlatform,
    OperationExecutor, OperationKind, PlatformAdapter, SavedExecutionReportSummary,
    SimulatedExecutor, SimulationConfig,
};
use std::env;
use std::path::Path;
use std::process::ExitCode;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::from(1)
        }
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let options = CliOptions::parse(raw_args)?;
    let args = options.args.as_slice();
    let cwd = env::current_dir()?;
    match args {
        [] => {
            print_help();
        }
        [cmd] if cmd == "help" || cmd == "--help" || cmd == "-h" => {
            print_help();
        }
        [cmd, sub] if cmd == "system" && sub == "inspect" => {
            let adapter = HostPlatform;
            let inventory = adapter.inspect()?;
            if options.json {
                println!("{}", inventory.to_json(adapter.name()));
            } else {
                println!("adapter: {}", adapter.name());
                println!("os: {}", inventory.os_name);
                println!("arch: {}", inventory.arch);
                println!("firmware: {:?}", inventory.firmware);
                println!("secure_boot: {:?}", inventory.secure_boot);
                println!("encryption: {:?}", inventory.encryption);
                for warning in inventory.warnings {
                    println!("warning: {warning}");
                }
            }
            write_journal_entry(&cwd, "system inspect")?;
        }
        [cmd, sub, scenario] if cmd == "system" && sub == "inspect-fake" => {
            let adapter = fake_adapter(scenario)?;
            let inventory = adapter.inspect()?;
            if options.json {
                println!("{}", inventory.to_json(adapter.name()));
            } else {
                println!("adapter: {}", adapter.name());
                println!("os: {}", inventory.os_name);
                println!("arch: {}", inventory.arch);
                println!("firmware: {:?}", inventory.firmware);
                println!("secure_boot: {:?}", inventory.secure_boot);
                println!("encryption: {:?}", inventory.encryption);
                println!(
                    "available_space_gb: {:?}",
                    inventory.disk_inventory.available_space_gb
                );
                println!("capabilities: {:?}", adapter.capabilities());
                for warning in inventory.warnings {
                    println!("warning: {warning}");
                }
            }
        }
        [cmd, sub] if cmd == "catalog" && sub == "list" => {
            let profiles = built_in_profiles();
            if options.json {
                let json_profiles: Vec<String> =
                    profiles.iter().map(|profile| profile.to_json()).collect();
                println!("[{}]", json_profiles.join(","));
            } else {
                for profile in profiles {
                    println!(
                        "{} | {} {} | {}",
                        profile.id, profile.name, profile.version, profile.source_url
                    );
                }
            }
        }
        [cmd, sub] if cmd == "catalog" && sub == "validate" => {
            let profiles = embedded_profiles()?;
            if options.json {
                println!(
                    "{{\"valid\":true,\"profile_count\":{},\"profile_ids\":[{}]}}",
                    profiles.len(),
                    profiles
                        .iter()
                        .map(|profile| format!("\"{}\"", profile.id))
                        .collect::<Vec<_>>()
                        .join(",")
                );
            } else {
                println!("catalog valid: {} profiles", profiles.len());
                for profile in profiles {
                    println!("- {}", profile.id);
                }
            }
        }
        [cmd, sub, profile] if cmd == "plan" => {
            let kind = parse_plan_kind(sub)?;
            let plan = plan_operation(kind, Some(profile))?;
            if options.json {
                println!("{}", plan.to_json());
            } else {
                if options.dry_run {
                    println!("Dry run: no execution will be attempted.");
                }
                println!("{}", plan.preview());
            }
            write_journal_entry(
                &cwd,
                &format!("planned {} dry_run={}", plan.id, options.dry_run),
            )?;
        }
        [cmd, scenario, plan_kind, profile] if cmd == "plan-fake" => {
            let adapter = fake_adapter(scenario)?;
            let inventory = adapter.inspect()?;
            let kind = parse_plan_kind(plan_kind)?;
            let plan = plan_operation_for_inventory(kind, Some(profile), &inventory)?;
            if options.json {
                println!("{}", plan.to_json());
            } else {
                if options.dry_run {
                    println!("Dry run: fake scenario only; no execution will be attempted.");
                }
                println!("Scenario: {}", adapter.name());
                println!("{}", plan.preview());
            }
            write_journal_entry(
                &cwd,
                &format!("planned fake scenario={} plan={}", adapter.name(), plan.id),
            )?;
        }
        [cmd, plan_kind, profile] if cmd == "simulate" => {
            let kind = parse_plan_kind(plan_kind)?;
            let plan = plan_operation(kind, Some(profile))?;
            let report = run_simulation(&plan, &options)?;
            print_execution_report(&report, options.json);
            let path = save_execution_report(&cwd, &report)?;
            if !options.json {
                println!("Saved report: {}", path.display());
            }
        }
        [cmd, scenario, plan_kind, profile] if cmd == "simulate-fake" => {
            let adapter = fake_adapter(scenario)?;
            let inventory = adapter.inspect()?;
            let kind = parse_plan_kind(plan_kind)?;
            let plan = plan_operation_for_inventory(kind, Some(profile), &inventory)?;
            let report = run_simulation(&plan, &options)?;
            if !options.json {
                println!("Scenario: {}", adapter.name());
            }
            print_execution_report(&report, options.json);
            let path = save_execution_report(&cwd, &report)?;
            write_journal_entry(
                &cwd,
                &format!(
                    "simulated fake scenario={} report={}",
                    adapter.name(),
                    path.display()
                ),
            )?;
            if !options.json {
                println!("Saved report: {}", path.display());
            }
        }
        [cmd, sub] if cmd == "recovery-pack" && sub == "create" => {
            let inventory = HostPlatform.inspect()?;
            let path = create_recovery_pack(&cwd, &inventory)?;
            write_journal_entry(&cwd, &format!("created recovery pack {}", path.display()))?;
            if options.json {
                println!("{{\"path\":\"{}\"}}", path.display());
            } else {
                println!("{}", path.display());
            }
        }
        [cmd, sub, path, expected] if cmd == "image" && sub == "verify" => {
            let ok = verify_sha256_with_system_tool(Path::new(path), expected)?;
            if ok {
                println!("verified: {path}");
            } else {
                println!("verification failed: {path}");
                return Err("checksum mismatch".into());
            }
        }
        [cmd, sub] if cmd == "logs" && sub == "export" => {
            let sqlite_path = cwd
                .join(".nousb")
                .join("journal")
                .join("operation_journal.sqlite3");
            let log_path = cwd.join(".nousb").join("journal").join("operations.log");
            if sqlite_path.exists() {
                println!("{}", sqlite_path.display());
            } else if log_path.exists() {
                println!("{}", log_path.display());
            } else {
                println!("no logs yet");
            }
        }
        [cmd, sub] if cmd == "reports" && sub == "list" => {
            let reports = list_saved_execution_reports(&cwd)?;
            if options.json {
                println!("{}", report_summaries_json(&reports));
            } else if reports.is_empty() {
                println!("no saved execution reports");
            } else {
                for report in reports {
                    println!(
                        "{} | {} | recovery_required={} | {}",
                        report.plan_id,
                        report.status,
                        report.recovery_required,
                        report.path.display()
                    );
                }
            }
        }
        [cmd, sub, plan_id] if cmd == "reports" && sub == "show" => {
            let report = load_execution_report_json(&cwd, plan_id)?;
            println!("{report}");
        }
        [cmd, sub] if cmd == "diagnostics" && sub == "export" => {
            let inventory = HostPlatform.inspect()?;
            let bundle = export_diagnostic_bundle(&cwd, &inventory)?;
            if options.json {
                println!("{}", bundle.to_json());
            } else {
                println!("Diagnostic bundle: {}", bundle.path.display());
                for file in bundle.files {
                    println!("- {}", file.display());
                }
            }
        }
        [cmd, sub] if cmd == "dashboard" && sub == "export" => {
            println!("{}", dashboard_export_json(&cwd)?);
        }
        [cmd] if cmd == "doctor" => {
            let inventory = HostPlatform.inspect()?;
            if options.json {
                println!(
                    "{{\"safe_default_build\":\"ok\",\"host\":\"{} {}\",\"real_boot_adapters\":\"disabled\"}}",
                    inventory.os_name, inventory.arch
                );
            } else {
                println!("NoUSB doctor");
                println!("safe default build: ok");
                println!("host: {} {}", inventory.os_name, inventory.arch);
                println!("real boot adapters: disabled");
            }
        }
        [cmd, plan_kind, profile] if cmd == "execute" => {
            let kind = parse_plan_kind(plan_kind)?;
            let plan = plan_operation(kind, Some(profile))?;
            let outcome = execute_plan_safe(&plan, options.yes)?;
            if options.json {
                println!("{}", outcome.to_json());
            } else {
                println!("{}", outcome.message);
            }
        }
        [cmd, plan_id] if cmd == "execute" => {
            return Err(format!("execute now expects <plan-kind> <profile>; got {plan_id}").into());
        }
        [cmd, operation_id] if cmd == "rollback" => {
            return Err(format!(
                "automatic rollback for {operation_id} requires a real adapter journal; see recovery pack"
            )
            .into());
        }
        [cmd, sub, _profile] if cmd == "image" && sub == "download" => {
            return Err("downloads are not enabled in this offline-safe scaffold; use official vendor pages listed by catalog".into());
        }
        _ => {
            print_help();
            return Err("unknown command".into());
        }
    }
    Ok(())
}

fn parse_plan_kind(value: &str) -> Result<OperationKind, Box<dyn std::error::Error>> {
    match value {
        "boot-once" => Ok(OperationKind::BootOnce),
        "dual-boot" => Ok(OperationKind::DualBoot),
        "replace-os" => Ok(OperationKind::ReplaceOs),
        "reinstall-current-os" => Ok(OperationKind::ReinstallCurrentOs),
        _ => Err(format!("unknown plan kind: {value}").into()),
    }
}

fn fake_adapter(value: &str) -> Result<FakePlatformAdapter, Box<dyn std::error::Error>> {
    match value {
        "windows-bitlocker" => Ok(FakePlatformAdapter::windows_uefi_bitlocker()),
        "linux-luks-low-space" => Ok(FakePlatformAdapter::linux_grub_luks_btrfs_low_space()),
        "macos-apple-silicon" => Ok(FakePlatformAdapter::macos_apple_silicon_filevault()),
        _ => Err(format!(
            "unknown fake scenario: {value}; expected windows-bitlocker, linux-luks-low-space, or macos-apple-silicon"
        )
        .into()),
    }
}

fn run_simulation(
    plan: &nousb_core::OperationPlan,
    options: &CliOptions,
) -> Result<nousb_core::ExecutionReport, Box<dyn std::error::Error>> {
    let config = if let Some(step_id) = &options.fail_step {
        SimulationConfig::fail_step(step_id.clone())
    } else if let Some(count) = options.interrupt_after {
        SimulationConfig::interrupt_after(count)
    } else {
        SimulationConfig::complete()
    };
    let report = SimulatedExecutor::new(config).execute(plan)?;
    if options.rollback {
        Ok(rollback_simulated_report(&report))
    } else {
        Ok(report)
    }
}

fn print_execution_report(report: &nousb_core::ExecutionReport, json: bool) {
    if json {
        println!("{}", report.to_json());
    } else {
        println!("{}", report.preview());
    }
}

fn report_summaries_json(reports: &[SavedExecutionReportSummary]) -> String {
    let reports: Vec<String> = reports.iter().map(|report| report.to_json()).collect();
    format!("[{}]", reports.join(","))
}

fn dashboard_export_json(base_dir: &Path) -> Result<String, Box<dyn std::error::Error>> {
    let profiles: Vec<String> = built_in_profiles()
        .iter()
        .map(|profile| profile.to_json())
        .collect();

    let fake_scenarios = [
        (
            "windows-bitlocker",
            FakePlatformAdapter::windows_uefi_bitlocker(),
        ),
        (
            "linux-luks-low-space",
            FakePlatformAdapter::linux_grub_luks_btrfs_low_space(),
        ),
        (
            "macos-apple-silicon",
            FakePlatformAdapter::macos_apple_silicon_filevault(),
        ),
    ];
    let scenario_json: Vec<String> = fake_scenarios
        .iter()
        .map(|(id, adapter)| {
            let inventory = adapter.inspect().expect("fake adapter inventory");
            format!(
                "{{\"id\":\"{}\",\"adapter\":\"{}\",\"inventory\":{}}}",
                id,
                adapter.name(),
                inventory.to_json(adapter.name())
            )
        })
        .collect();

    let linux_inventory = FakePlatformAdapter::linux_grub_luks_btrfs_low_space().inspect()?;
    let sample_plan = plan_operation_for_inventory(
        OperationKind::BootOnce,
        Some("ubuntu-24.04-desktop"),
        &linux_inventory,
    )?;

    let commands = [
        "cargo run -p nousb -- catalog validate",
        "cargo run -p nousb -- system inspect-fake windows-bitlocker",
        "cargo run -p nousb -- plan-fake linux-luks-low-space boot-once ubuntu-24.04-desktop",
        "cargo run -p nousb -- --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop",
        "cargo run -p nousb -- reports list",
        "cargo run -p nousb -- diagnostics export",
        "cargo run -p nousb -- dashboard export",
    ];
    let commands: Vec<String> = commands
        .iter()
        .map(|command| format!("\"{}\"", escape_json_for_cli(command)))
        .collect();

    let reports = list_saved_execution_reports(base_dir)?;

    Ok(format!(
        "{{\"generated_by\":\"nousb dashboard export\",\"safe_default_build\":true,\"catalog\":[{}],\"fake_scenarios\":[{}],\"sample_plan\":{},\"reports\":{},\"commands\":[{}]}}",
        profiles.join(","),
        scenario_json.join(","),
        sample_plan.to_json(),
        report_summaries_json(&reports),
        commands.join(",")
    ))
}

fn escape_json_for_cli(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

struct CliOptions {
    json: bool,
    dry_run: bool,
    yes: bool,
    rollback: bool,
    interrupt_after: Option<usize>,
    fail_step: Option<String>,
    args: Vec<String>,
}

impl CliOptions {
    fn parse(raw_args: Vec<String>) -> Result<Self, Box<dyn std::error::Error>> {
        let mut json = false;
        let mut dry_run = false;
        let mut yes = false;
        let mut rollback = false;
        let mut interrupt_after = None;
        let mut fail_step = None;
        let mut args = Vec::new();
        let mut index = 0usize;

        while index < raw_args.len() {
            let arg = &raw_args[index];
            match arg.as_str() {
                "--json" => json = true,
                "--dry-run" => dry_run = true,
                "--yes" | "-y" => yes = true,
                "--rollback" => rollback = true,
                "--interrupt-after" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err("--interrupt-after requires a count".into());
                    };
                    interrupt_after = Some(value.parse()?);
                }
                "--fail-step" => {
                    index += 1;
                    let Some(value) = raw_args.get(index) else {
                        return Err("--fail-step requires a step id".into());
                    };
                    fail_step = Some(value.clone());
                }
                "--" => {}
                value if value.starts_with('-') => {
                    return Err(format!("unknown option: {value}").into());
                }
                _ => args.push(arg.clone()),
            }
            index += 1;
        }

        Ok(Self {
            json,
            dry_run,
            yes,
            rollback,
            interrupt_after,
            fail_step,
            args,
        })
    }
}

fn print_help() {
    println!("NoUSB OS Switcher");
    println!();
    println!("Commands:");
    println!("  nousb system inspect");
    println!("  nousb system inspect-fake <scenario>");
    println!("  nousb catalog list");
    println!("  nousb catalog validate");
    println!("  nousb image download <profile>");
    println!("  nousb image verify <path> <sha256>");
    println!("  nousb plan boot-once <profile>");
    println!("  nousb plan dual-boot <profile>");
    println!("  nousb plan replace-os <profile>");
    println!("  nousb plan reinstall-current-os <profile>");
    println!("  nousb plan-fake <scenario> <plan-kind> <profile>");
    println!("  nousb simulate <plan-kind> <profile>");
    println!("  nousb simulate-fake <scenario> <plan-kind> <profile>");
    println!("  nousb reports list");
    println!("  nousb reports show <plan-id>");
    println!("  nousb diagnostics export");
    println!("  nousb dashboard export");
    println!("  nousb execute <plan-kind> <profile>");
    println!("  nousb rollback <operation-id>");
    println!("  nousb recovery-pack create");
    println!("  nousb logs export");
    println!("  nousb doctor");
    println!();
    println!("Options:");
    println!("  --json       Print machine-readable JSON where supported");
    println!("  --dry-run    Mark plan output as a dry run");
    println!("  --yes        Only allowed for low-risk actions; never bypasses typed confirmation");
    println!("  --interrupt-after <count>");
    println!("  --fail-step <step-id>");
    println!("  --rollback   Roll back the simulated report");
    println!();
    println!("Fake scenarios:");
    println!("  windows-bitlocker");
    println!("  linux-luks-low-space");
    println!("  macos-apple-silicon");
}
