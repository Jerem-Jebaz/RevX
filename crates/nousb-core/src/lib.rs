use std::fmt;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum FirmwareMode {
    Uefi,
    Bios,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SecureBootState {
    Enabled,
    Disabled,
    Unsupported,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum EncryptionState {
    Enabled(String),
    Disabled,
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PartitionInfo {
    pub name: String,
    pub mount_point: Option<String>,
    pub filesystem: Option<String>,
    pub size_bytes: Option<u64>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiskInventory {
    pub disks: Vec<String>,
    pub partitions: Vec<PartitionInfo>,
    pub available_space_gb: Option<u32>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootEntry {
    pub id: String,
    pub label: String,
    pub is_default: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SystemInventory {
    pub os_name: String,
    pub arch: String,
    pub firmware: FirmwareMode,
    pub secure_boot: SecureBootState,
    pub encryption: EncryptionState,
    pub disk_inventory: DiskInventory,
    pub boot_entries: Vec<BootEntry>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StagingStrategy {
    ExtractedEfiInstaller,
    IsoLoopback,
    KernelInitrd,
    WindowsMountedIsoSetup,
    MacosStartosinstall,
    ManualInstructions,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlatformCapability {
    OneTimeBoot,
    BootEntryBackup,
    RecoveryPack,
    Snapshot,
    EncryptionStatus,
    SecureBootDetection,
    InstallerVerification,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InstallerProfile {
    pub schema_version: String,
    pub id: String,
    pub name: String,
    pub vendor: String,
    pub version: String,
    pub architecture: String,
    pub source_url: String,
    pub checksum_url: String,
    pub sha256: String,
    pub signature_url: Option<String>,
    pub homepage: String,
    pub license_notes: String,
    pub supported_platforms: Vec<String>,
    pub supported_staging_strategies: Vec<StagingStrategy>,
    pub required_free_space_gb: u32,
    pub extraction_rules: Vec<String>,
    pub kernel_initrd_paths: Vec<String>,
    pub bootloader_paths: Vec<String>,
    pub secure_boot_notes: String,
    pub known_limitations: Vec<String>,
    pub post_boot_instructions: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum RiskLevel {
    Info,
    Low,
    Medium,
    High,
    Critical,
}

impl fmt::Display for RiskLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            RiskLevel::Info => "info",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        };
        f.write_str(value)
    }
}

impl RiskLevel {
    pub fn as_str(&self) -> &'static str {
        match self {
            RiskLevel::Info => "info",
            RiskLevel::Low => "low",
            RiskLevel::Medium => "medium",
            RiskLevel::High => "high",
            RiskLevel::Critical => "critical",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ConfirmationRequirement {
    None,
    SimpleYes,
    TypePhrase(String),
    TypeTargetLabel(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RollbackAction {
    None,
    RemoveStagedFiles,
    RestoreBootBackup,
    Manual(String),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Completed,
    Failed,
    RolledBack,
    ManualInterventionRequired,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationStep {
    pub id: String,
    pub title: String,
    pub description: String,
    pub risk: RiskLevel,
    pub modifies: Vec<String>,
    pub rollback: RollbackAction,
    pub state: StepState,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BootPlanPreference {
    OneTimeBoot,
    TemporaryBootEntryWithFallback,
    ExistingBootManagerOneShot,
    ManualInstructionsOnly,
}

impl BootPlanPreference {
    pub fn as_str(&self) -> &'static str {
        match self {
            BootPlanPreference::OneTimeBoot => "one_time_boot",
            BootPlanPreference::TemporaryBootEntryWithFallback => {
                "temporary_boot_entry_with_fallback"
            }
            BootPlanPreference::ExistingBootManagerOneShot => "existing_boot_manager_one_shot",
            BootPlanPreference::ManualInstructionsOnly => "manual_instructions_only",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BootPlan {
    pub preference_order: Vec<BootPlanPreference>,
    pub selected_preference: BootPlanPreference,
    pub keep_current_os_default: bool,
    pub allow_permanent_boot_order_change: bool,
    pub fallback_description: String,
    pub backup_requirements: Vec<String>,
    pub manual_instructions: Vec<String>,
}

impl BootPlan {
    pub fn safe_default(kind: &OperationKind) -> Self {
        let selected_preference = match kind {
            OperationKind::BootOnce => BootPlanPreference::OneTimeBoot,
            OperationKind::DualBoot
            | OperationKind::ReplaceOs
            | OperationKind::ReinstallCurrentOs => {
                BootPlanPreference::TemporaryBootEntryWithFallback
            }
            OperationKind::RecoveryPack => BootPlanPreference::ManualInstructionsOnly,
        };

        Self {
            preference_order: vec![
                BootPlanPreference::OneTimeBoot,
                BootPlanPreference::TemporaryBootEntryWithFallback,
                BootPlanPreference::ExistingBootManagerOneShot,
                BootPlanPreference::ManualInstructionsOnly,
            ],
            selected_preference,
            keep_current_os_default: true,
            allow_permanent_boot_order_change: false,
            fallback_description:
                "Keep the current OS as the default boot target; if staged boot fails, firmware should return to the existing OS path."
                    .into(),
            backup_requirements: vec![
                "Back up existing boot entries before editing boot metadata.".into(),
                "Back up bootloader or BCD configuration before any real adapter executes.".into(),
                "Back up touched EFI System Partition files before modification.".into(),
            ],
            manual_instructions: vec![
                "Use firmware boot menu to return to the current OS if automated one-time boot is unavailable.".into(),
                "Do not make installer media the permanent default in the safe default build.".into(),
            ],
        }
    }

    pub fn to_json(&self) -> String {
        let preference_order: Vec<String> = self
            .preference_order
            .iter()
            .map(|preference| preference.as_str().to_string())
            .collect();
        format!(
            "{{\"preference_order\":{},\"selected_preference\":\"{}\",\"keep_current_os_default\":{},\"allow_permanent_boot_order_change\":{}, {},\"backup_requirements\":{},\"manual_instructions\":{}}}",
            json_string_array(&preference_order),
            self.selected_preference.as_str(),
            self.keep_current_os_default,
            self.allow_permanent_boot_order_change,
            json_pair("fallback_description", &self.fallback_description),
            json_string_array(&self.backup_requirements),
            json_string_array(&self.manual_instructions)
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OperationKind {
    BootOnce,
    DualBoot,
    ReplaceOs,
    ReinstallCurrentOs,
    RecoveryPack,
}

impl fmt::Display for OperationKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let value = match self {
            OperationKind::BootOnce => "boot-once",
            OperationKind::DualBoot => "dual-boot",
            OperationKind::ReplaceOs => "replace-os",
            OperationKind::ReinstallCurrentOs => "reinstall-current-os",
            OperationKind::RecoveryPack => "recovery-pack",
        };
        f.write_str(value)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct OperationPlan {
    pub id: String,
    pub kind: OperationKind,
    pub profile_id: Option<String>,
    pub risk: RiskLevel,
    pub required_privileges: Vec<String>,
    pub confirmation: ConfirmationRequirement,
    pub warnings: Vec<String>,
    pub boot_plan: Option<BootPlan>,
    pub steps: Vec<OperationStep>,
}

impl OperationPlan {
    pub fn preview(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!("Plan: {}\nRisk: {}\n", self.id, self.risk));
        if !self.required_privileges.is_empty() {
            output.push_str(&format!(
                "Privileges: {}\n",
                self.required_privileges.join(", ")
            ));
        }
        if !self.warnings.is_empty() {
            output.push_str("Warnings:\n");
            for warning in &self.warnings {
                output.push_str(&format!("- {warning}\n"));
            }
        }
        if let Some(boot_plan) = &self.boot_plan {
            output.push_str("Boot safety:\n");
            output.push_str(&format!(
                "- selected: {}\n- keep current OS default: {}\n- permanent boot order change allowed: {}\n",
                boot_plan.selected_preference.as_str(),
                boot_plan.keep_current_os_default,
                boot_plan.allow_permanent_boot_order_change
            ));
        }
        output.push_str("Steps:\n");
        for step in &self.steps {
            output.push_str(&format!("- [{}] {}: {}\n", step.risk, step.id, step.title));
            if !step.modifies.is_empty() {
                output.push_str(&format!("  modifies: {}\n", step.modifies.join(", ")));
            }
        }
        output
    }

    pub fn to_json(&self) -> String {
        let mut output = String::new();
        output.push_str("{");
        output.push_str(&json_pair("id", &self.id));
        output.push_str(&format!(",\"kind\":\"{}\"", self.kind));
        match &self.profile_id {
            Some(profile_id) => {
                output.push_str(&format!(",{}", json_pair("profile_id", profile_id)))
            }
            None => output.push_str(",\"profile_id\":null"),
        }
        output.push_str(&format!(",\"risk\":\"{}\"", self.risk.as_str()));
        output.push_str(",\"required_privileges\":");
        output.push_str(&json_string_array(&self.required_privileges));
        output.push_str(",\"confirmation\":");
        output.push_str(&self.confirmation.to_json());
        output.push_str(",\"warnings\":");
        output.push_str(&json_string_array(&self.warnings));
        output.push_str(",\"boot_plan\":");
        match &self.boot_plan {
            Some(boot_plan) => output.push_str(&boot_plan.to_json()),
            None => output.push_str("null"),
        }
        output.push_str(",\"steps\":[");
        for (index, step) in self.steps.iter().enumerate() {
            if index > 0 {
                output.push(',');
            }
            output.push_str(&step.to_json());
        }
        output.push_str("]}");
        output
    }
}

impl ConfirmationRequirement {
    pub fn permits_yes_flag(&self, risk: RiskLevel) -> bool {
        matches!(
            self,
            ConfirmationRequirement::None | ConfirmationRequirement::SimpleYes
        ) && risk <= RiskLevel::Low
    }

    pub fn to_json(&self) -> String {
        match self {
            ConfirmationRequirement::None => "{\"type\":\"none\"}".into(),
            ConfirmationRequirement::SimpleYes => "{\"type\":\"simple_yes\"}".into(),
            ConfirmationRequirement::TypePhrase(phrase) => {
                format!(
                    "{{\"type\":\"type_phrase\",{}}}",
                    json_pair("phrase", phrase)
                )
            }
            ConfirmationRequirement::TypeTargetLabel(label) => {
                format!(
                    "{{\"type\":\"type_target_label\",{}}}",
                    json_pair("label", label)
                )
            }
        }
    }
}

impl OperationStep {
    pub fn to_json(&self) -> String {
        format!(
            "{{{},{},{},\"risk\":\"{}\",\"modifies\":{},\"rollback\":{},\"state\":\"{}\"}}",
            json_pair("id", &self.id),
            json_pair("title", &self.title),
            json_pair("description", &self.description),
            self.risk.as_str(),
            json_string_array(&self.modifies),
            self.rollback.to_json(),
            self.state.as_str()
        )
    }
}

impl StepState {
    pub fn as_str(&self) -> &'static str {
        match self {
            StepState::Pending => "pending",
            StepState::Running => "running",
            StepState::Completed => "completed",
            StepState::Failed => "failed",
            StepState::RolledBack => "rolled_back",
            StepState::ManualInterventionRequired => "manual_intervention_required",
        }
    }
}

impl RollbackAction {
    pub fn to_json(&self) -> String {
        match self {
            RollbackAction::None => "{\"type\":\"none\"}".into(),
            RollbackAction::RemoveStagedFiles => "{\"type\":\"remove_staged_files\"}".into(),
            RollbackAction::RestoreBootBackup => "{\"type\":\"restore_boot_backup\"}".into(),
            RollbackAction::Manual(note) => {
                format!("{{\"type\":\"manual\",{}}}", json_pair("note", note))
            }
        }
    }
}

impl InstallerProfile {
    pub fn to_json(&self) -> String {
        let strategies: Vec<String> = self
            .supported_staging_strategies
            .iter()
            .map(|strategy| strategy.as_str().to_string())
            .collect();
        format!(
            "{{{},{},{},{},{},{},{},{},{},\"signature_url\":{}, {},{},\"supported_platforms\":{},\"supported_staging_strategies\":{},\"required_free_space_gb\":{},\"extraction_rules\":{},\"kernel_initrd_paths\":{},\"bootloader_paths\":{}, {},\"known_limitations\":{}, {}}}",
            json_pair("schema_version", &self.schema_version),
            json_pair("id", &self.id),
            json_pair("name", &self.name),
            json_pair("vendor", &self.vendor),
            json_pair("version", &self.version),
            json_pair("architecture", &self.architecture),
            json_pair("source_url", &self.source_url),
            json_pair("checksum_url", &self.checksum_url),
            json_pair("sha256", &self.sha256),
            json_optional_value(self.signature_url.as_deref()),
            json_pair("homepage", &self.homepage),
            json_pair("license_notes", &self.license_notes),
            json_string_array(&self.supported_platforms),
            json_string_array(&strategies),
            self.required_free_space_gb,
            json_string_array(&self.extraction_rules),
            json_string_array(&self.kernel_initrd_paths),
            json_string_array(&self.bootloader_paths),
            json_pair("secure_boot_notes", &self.secure_boot_notes),
            json_string_array(&self.known_limitations),
            json_pair("post_boot_instructions", &self.post_boot_instructions)
        )
    }
}

impl StagingStrategy {
    pub fn as_str(&self) -> &'static str {
        match self {
            StagingStrategy::ExtractedEfiInstaller => "extracted_efi_installer",
            StagingStrategy::IsoLoopback => "iso_loopback",
            StagingStrategy::KernelInitrd => "kernel_initrd",
            StagingStrategy::WindowsMountedIsoSetup => "windows_mounted_iso_setup",
            StagingStrategy::MacosStartosinstall => "macos_startosinstall",
            StagingStrategy::ManualInstructions => "manual_instructions",
        }
    }

    pub fn from_profile_value(value: &str) -> Result<Self, NoUsbError> {
        match value {
            "extracted_efi_installer" => Ok(StagingStrategy::ExtractedEfiInstaller),
            "iso_loopback" => Ok(StagingStrategy::IsoLoopback),
            "kernel_initrd" => Ok(StagingStrategy::KernelInitrd),
            "windows_mounted_iso_setup" => Ok(StagingStrategy::WindowsMountedIsoSetup),
            "macos_startosinstall" => Ok(StagingStrategy::MacosStartosinstall),
            "manual_instructions" => Ok(StagingStrategy::ManualInstructions),
            _ => Err(NoUsbError::ProfileParse(format!(
                "unknown staging strategy: {value}"
            ))),
        }
    }
}

impl SystemInventory {
    pub fn to_json(&self, adapter: &str) -> String {
        let partitions: Vec<String> = self
            .disk_inventory
            .partitions
            .iter()
            .map(|partition| {
                format!(
                    "{{{},{},\"filesystem\":{},\"size_bytes\":{}}}",
                    json_pair("name", &partition.name),
                    json_optional_pair("mount_point", partition.mount_point.as_deref()),
                    json_optional_value(partition.filesystem.as_deref()),
                    partition
                        .size_bytes
                        .map(|size| size.to_string())
                        .unwrap_or_else(|| "null".into())
                )
            })
            .collect();
        let boot_entries: Vec<String> = self
            .boot_entries
            .iter()
            .map(|entry| {
                format!(
                    "{{{},{},\"is_default\":{}}}",
                    json_pair("id", &entry.id),
                    json_pair("label", &entry.label),
                    entry.is_default
                )
            })
            .collect();
        format!(
            "{{{},{},{},{},{},{},\"disk_inventory\":{{\"disks\":{},\"available_space_gb\":{},\"partitions\":[{}]}},\"boot_entries\":[{}],\"warnings\":{}}}",
            json_pair("adapter", adapter),
            json_pair("os_name", &self.os_name),
            json_pair("arch", &self.arch),
            json_pair("firmware", &format!("{:?}", self.firmware)),
            json_pair("secure_boot", &format!("{:?}", self.secure_boot)),
            json_pair("encryption", &format!("{:?}", self.encryption)),
            json_string_array(&self.disk_inventory.disks),
            self.disk_inventory
                .available_space_gb
                .map(|value| value.to_string())
                .unwrap_or_else(|| "null".into()),
            partitions.join(","),
            boot_entries.join(","),
            json_string_array(&self.warnings)
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionOutcome {
    pub plan_id: String,
    pub executed: bool,
    pub message: String,
}

impl ExecutionOutcome {
    pub fn to_json(&self) -> String {
        format!(
            "{{{},\"executed\":{}, {}}}",
            json_pair("plan_id", &self.plan_id),
            self.executed,
            json_pair("message", &self.message)
        )
    }
}

pub trait PlatformAdapter {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError>;
    fn name(&self) -> &'static str;
}

pub trait OperationExecutor {
    fn execute(&self, plan: &OperationPlan) -> Result<ExecutionReport, NoUsbError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SimulationConfig {
    pub interrupt_after_completed_steps: Option<usize>,
    pub fail_step_id: Option<String>,
}

impl SimulationConfig {
    pub fn complete() -> Self {
        Self {
            interrupt_after_completed_steps: None,
            fail_step_id: None,
        }
    }

    pub fn interrupt_after(completed_steps: usize) -> Self {
        Self {
            interrupt_after_completed_steps: Some(completed_steps),
            fail_step_id: None,
        }
    }

    pub fn fail_step(step_id: impl Into<String>) -> Self {
        Self {
            interrupt_after_completed_steps: None,
            fail_step_id: Some(step_id.into()),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExecutionStatus {
    Completed,
    Interrupted,
    Failed,
    RolledBack,
}

impl ExecutionStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExecutionStatus::Completed => "completed",
            ExecutionStatus::Interrupted => "interrupted",
            ExecutionStatus::Failed => "failed",
            ExecutionStatus::RolledBack => "rolled_back",
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExecutionReport {
    pub plan_id: String,
    pub status: ExecutionStatus,
    pub steps: Vec<OperationStep>,
    pub journal: Vec<String>,
    pub recovery_required: bool,
}

impl ExecutionReport {
    pub fn preview(&self) -> String {
        let mut output = String::new();
        output.push_str(&format!(
            "Execution report: {}\nStatus: {}\nRecovery required: {}\n",
            self.plan_id,
            self.status.as_str(),
            self.recovery_required
        ));
        output.push_str("Steps:\n");
        for step in &self.steps {
            output.push_str(&format!("- {}: {}\n", step.id, step.state.as_str()));
        }
        if !self.journal.is_empty() {
            output.push_str("Journal:\n");
            for entry in &self.journal {
                output.push_str(&format!("- {entry}\n"));
            }
        }
        output
    }

    pub fn to_json(&self) -> String {
        let steps: Vec<String> = self.steps.iter().map(|step| step.to_json()).collect();
        format!(
            "{{{},\"status\":\"{}\",\"steps\":[{}],\"journal\":{},\"recovery_required\":{}}}",
            json_pair("plan_id", &self.plan_id),
            self.status.as_str(),
            steps.join(","),
            json_string_array(&self.journal),
            self.recovery_required
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedExecutionReportSummary {
    pub plan_id: String,
    pub status: String,
    pub recovery_required: bool,
    pub path: PathBuf,
}

impl SavedExecutionReportSummary {
    pub fn to_json(&self) -> String {
        format!(
            "{{{},{},\"recovery_required\":{}, {}}}",
            json_pair("plan_id", &self.plan_id),
            json_pair("status", &self.status),
            self.recovery_required,
            json_pair("path", &self.path.display().to_string())
        )
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DiagnosticBundle {
    pub path: PathBuf,
    pub files: Vec<PathBuf>,
}

impl DiagnosticBundle {
    pub fn to_json(&self) -> String {
        let files: Vec<String> = self
            .files
            .iter()
            .map(|path| path.display().to_string())
            .collect();
        format!(
            "{{{},\"files\":{}}}",
            json_pair("path", &self.path.display().to_string()),
            json_string_array(&files)
        )
    }
}

pub struct SimulatedExecutor {
    config: SimulationConfig,
}

impl SimulatedExecutor {
    pub fn new(config: SimulationConfig) -> Self {
        Self { config }
    }
}

impl OperationExecutor for SimulatedExecutor {
    fn execute(&self, plan: &OperationPlan) -> Result<ExecutionReport, NoUsbError> {
        let mut steps = plan.steps.clone();
        let mut journal = vec![format!("simulation started for {}", plan.id)];
        let mut completed = 0usize;

        for step in &mut steps {
            if step.state == StepState::ManualInterventionRequired {
                journal.push(format!(
                    "step {} requires manual intervention before execution",
                    step.id
                ));
                return Ok(ExecutionReport {
                    plan_id: plan.id.clone(),
                    status: ExecutionStatus::Interrupted,
                    steps,
                    journal,
                    recovery_required: true,
                });
            }

            step.state = StepState::Running;
            journal.push(format!("step {} running", step.id));

            if self.config.fail_step_id.as_deref() == Some(step.id.as_str()) {
                step.state = StepState::Failed;
                journal.push(format!("step {} failed by simulation", step.id));
                return Ok(ExecutionReport {
                    plan_id: plan.id.clone(),
                    status: ExecutionStatus::Failed,
                    steps,
                    journal,
                    recovery_required: true,
                });
            }

            step.state = StepState::Completed;
            completed += 1;
            journal.push(format!("step {} completed", step.id));

            if self.config.interrupt_after_completed_steps == Some(completed) {
                journal.push(format!(
                    "simulation interrupted after {completed} completed steps"
                ));
                return Ok(ExecutionReport {
                    plan_id: plan.id.clone(),
                    status: ExecutionStatus::Interrupted,
                    steps,
                    journal,
                    recovery_required: true,
                });
            }
        }

        journal.push("simulation completed".into());
        Ok(ExecutionReport {
            plan_id: plan.id.clone(),
            status: ExecutionStatus::Completed,
            steps,
            journal,
            recovery_required: false,
        })
    }
}

pub fn rollback_simulated_report(report: &ExecutionReport) -> ExecutionReport {
    let mut steps = report.steps.clone();
    let mut journal = report.journal.clone();
    journal.push(format!("rollback started for {}", report.plan_id));

    for step in steps.iter_mut().rev() {
        match step.state {
            StepState::Completed => match &step.rollback {
                RollbackAction::None => {
                    journal.push(format!("step {} has no rollback action", step.id));
                }
                _ => {
                    step.state = StepState::RolledBack;
                    journal.push(format!("step {} rolled back", step.id));
                }
            },
            StepState::Running | StepState::Failed => {
                step.state = StepState::ManualInterventionRequired;
                journal.push(format!(
                    "step {} needs manual intervention before rollback can continue",
                    step.id
                ));
            }
            _ => {}
        }
    }

    let recovery_required = steps
        .iter()
        .any(|step| step.state == StepState::ManualInterventionRequired);

    ExecutionReport {
        plan_id: report.plan_id.clone(),
        status: ExecutionStatus::RolledBack,
        steps,
        journal,
        recovery_required,
    }
}

pub fn save_execution_report(
    base_dir: &Path,
    report: &ExecutionReport,
) -> Result<PathBuf, NoUsbError> {
    let dir = execution_reports_dir(base_dir);
    fs::create_dir_all(&dir)?;
    let path = dir.join(format!("{}.json", safe_filename(&report.plan_id)));
    fs::write(&path, report.to_json())?;
    write_journal_entry(
        base_dir,
        &format!(
            "saved execution report {} status={}",
            report.plan_id,
            report.status.as_str()
        ),
    )?;
    Ok(path)
}

pub fn load_execution_report_json(base_dir: &Path, plan_id: &str) -> Result<String, NoUsbError> {
    let path = execution_reports_dir(base_dir).join(format!("{}.json", safe_filename(plan_id)));
    if !path.exists() {
        return Err(NoUsbError::InvalidInput(format!(
            "no saved execution report for {plan_id}"
        )));
    }
    Ok(fs::read_to_string(path)?)
}

pub fn list_saved_execution_reports(
    base_dir: &Path,
) -> Result<Vec<SavedExecutionReportSummary>, NoUsbError> {
    let dir = execution_reports_dir(base_dir);
    if !dir.exists() {
        return Ok(vec![]);
    }

    let mut reports = Vec::new();
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|value| value.to_str()) != Some("json") {
            continue;
        }
        let content = fs::read_to_string(&path)?;
        let plan_id = json_field(&content, "plan_id").unwrap_or_else(|| {
            path.file_stem()
                .and_then(|value| value.to_str())
                .unwrap_or("unknown")
                .to_string()
        });
        let status = json_field(&content, "status").unwrap_or_else(|| "unknown".into());
        let recovery_required = json_bool_field(&content, "recovery_required").unwrap_or(false);
        reports.push(SavedExecutionReportSummary {
            plan_id,
            status,
            recovery_required,
            path,
        });
    }
    reports.sort_by(|left, right| left.plan_id.cmp(&right.plan_id));
    Ok(reports)
}

fn execution_reports_dir(base_dir: &Path) -> PathBuf {
    base_dir.join(".nousb").join("reports")
}

pub fn export_diagnostic_bundle(
    base_dir: &Path,
    inventory: &SystemInventory,
) -> Result<DiagnosticBundle, NoUsbError> {
    let bundle_dir = base_dir
        .join(".nousb")
        .join("diagnostics")
        .join(format!("bundle-{}", now_stamp()));
    fs::create_dir_all(&bundle_dir)?;

    let mut files = Vec::new();
    write_diagnostic_file(
        &bundle_dir,
        &mut files,
        "README.txt",
        "NoUSB diagnostic bundle. Secrets are redacted with [REDACTED]. This bundle contains project-local logs, saved reports, catalog metadata, and safe host inventory only.\n",
    )?;
    write_diagnostic_file(
        &bundle_dir,
        &mut files,
        "system-inventory.json",
        &inventory.to_json("diagnostic-export"),
    )?;

    let catalog_json = format!(
        "[{}]",
        built_in_profiles()
            .iter()
            .map(|profile| profile.to_json())
            .collect::<Vec<_>>()
            .join(",")
    );
    write_diagnostic_file(&bundle_dir, &mut files, "catalog.json", &catalog_json)?;

    let reports = list_saved_execution_reports(base_dir)?;
    let report_summary = format!(
        "[{}]",
        reports
            .iter()
            .map(|report| report.to_json())
            .collect::<Vec<_>>()
            .join(",")
    );
    write_diagnostic_file(
        &bundle_dir,
        &mut files,
        "execution-report-index.json",
        &report_summary,
    )?;

    for report in reports {
        let content = fs::read_to_string(&report.path)?;
        write_diagnostic_file(
            &bundle_dir.join("reports"),
            &mut files,
            &format!("{}.json", safe_filename(&report.plan_id)),
            &content,
        )?;
    }

    let log_path = base_dir
        .join(".nousb")
        .join("journal")
        .join("operations.log");
    if log_path.exists() {
        let content = fs::read_to_string(log_path)?;
        write_diagnostic_file(&bundle_dir, &mut files, "operations.log", &content)?;
    }

    write_journal_entry(
        base_dir,
        &format!("exported diagnostic bundle {}", bundle_dir.display()),
    )?;

    Ok(DiagnosticBundle {
        path: bundle_dir,
        files,
    })
}

fn write_diagnostic_file(
    base_dir: &Path,
    files: &mut Vec<PathBuf>,
    name: &str,
    content: &str,
) -> Result<(), NoUsbError> {
    fs::create_dir_all(base_dir)?;
    let path = base_dir.join(name);
    fs::write(&path, redact_secrets(content))?;
    files.push(path);
    Ok(())
}

pub fn redact_secrets(content: &str) -> String {
    let mut redacted_lines = Vec::new();
    for line in content.lines() {
        redacted_lines.push(redact_secret_line(line));
    }
    redacted_lines.join("\n")
}

fn redact_secret_line(line: &str) -> String {
    let lower = line.to_ascii_lowercase();
    let secret_markers = [
        "password",
        "passphrase",
        "recovery_key",
        "recovery-key",
        "bitlocker",
        "filevault",
        "luks_header",
        "token",
        "secret",
        "api_key",
        "apikey",
        "private_key",
    ];
    if !secret_markers.iter().any(|marker| lower.contains(marker)) {
        return line.to_string();
    }

    if let Some((key, _)) = line.split_once('=') {
        format!("{}=[REDACTED]", key.trim_end())
    } else if let Some((key, _)) = line.split_once(':') {
        format!("{}: [REDACTED]", key.trim_end())
    } else {
        "[REDACTED]".into()
    }
}

#[derive(Clone, Debug)]
pub struct FakePlatformAdapter {
    name: &'static str,
    inventory: SystemInventory,
    capabilities: Vec<PlatformCapability>,
}

impl FakePlatformAdapter {
    pub fn windows_uefi_bitlocker() -> Self {
        Self {
            name: "fake-windows-uefi-bitlocker",
            inventory: SystemInventory {
                os_name: "windows".into(),
                arch: "x86_64".into(),
                firmware: FirmwareMode::Uefi,
                secure_boot: SecureBootState::Enabled,
                encryption: EncryptionState::Enabled("BitLocker".into()),
                disk_inventory: DiskInventory {
                    disks: vec!["Disk0 GPT 512GB".into()],
                    partitions: vec![
                        PartitionInfo {
                            name: "EFI System Partition".into(),
                            mount_point: Some("ESP".into()),
                            filesystem: Some("FAT32".into()),
                            size_bytes: Some(272 * 1024 * 1024),
                        },
                        PartitionInfo {
                            name: "Windows".into(),
                            mount_point: Some("C:\\".into()),
                            filesystem: Some("NTFS".into()),
                            size_bytes: Some(400 * 1024 * 1024 * 1024),
                        },
                    ],
                    available_space_gb: Some(64),
                },
                boot_entries: vec![BootEntry {
                    id: "{bootmgr}".into(),
                    label: "Windows Boot Manager".into(),
                    is_default: true,
                }],
                warnings: vec![
                    "BitLocker recovery key backup must be confirmed before boot changes.".into(),
                ],
            },
            capabilities: vec![
                PlatformCapability::OneTimeBoot,
                PlatformCapability::BootEntryBackup,
                PlatformCapability::RecoveryPack,
                PlatformCapability::EncryptionStatus,
                PlatformCapability::SecureBootDetection,
                PlatformCapability::InstallerVerification,
            ],
        }
    }

    pub fn linux_grub_luks_btrfs_low_space() -> Self {
        Self {
            name: "fake-linux-grub-luks-btrfs-low-space",
            inventory: SystemInventory {
                os_name: "linux".into(),
                arch: "x86_64".into(),
                firmware: FirmwareMode::Uefi,
                secure_boot: SecureBootState::Disabled,
                encryption: EncryptionState::Enabled("LUKS".into()),
                disk_inventory: DiskInventory {
                    disks: vec!["nvme0n1 GPT 256GB".into()],
                    partitions: vec![
                        PartitionInfo {
                            name: "EFI System Partition".into(),
                            mount_point: Some("/boot/efi".into()),
                            filesystem: Some("vfat".into()),
                            size_bytes: Some(512 * 1024 * 1024),
                        },
                        PartitionInfo {
                            name: "root".into(),
                            mount_point: Some("/".into()),
                            filesystem: Some("btrfs".into()),
                            size_bytes: Some(180 * 1024 * 1024 * 1024),
                        },
                    ],
                    available_space_gb: Some(3),
                },
                boot_entries: vec![BootEntry {
                    id: "0001".into(),
                    label: "GRUB".into(),
                    is_default: true,
                }],
                warnings: vec![
                    "LUKS header backup should be completed manually before boot work.".into(),
                    "Btrfs snapshot may be available before risky operations.".into(),
                ],
            },
            capabilities: vec![
                PlatformCapability::OneTimeBoot,
                PlatformCapability::BootEntryBackup,
                PlatformCapability::RecoveryPack,
                PlatformCapability::Snapshot,
                PlatformCapability::EncryptionStatus,
                PlatformCapability::InstallerVerification,
            ],
        }
    }

    pub fn macos_apple_silicon_filevault() -> Self {
        Self {
            name: "fake-macos-apple-silicon-filevault",
            inventory: SystemInventory {
                os_name: "macos".into(),
                arch: "aarch64".into(),
                firmware: FirmwareMode::Uefi,
                secure_boot: SecureBootState::Enabled,
                encryption: EncryptionState::Enabled("FileVault".into()),
                disk_inventory: DiskInventory {
                    disks: vec!["APFS container 1TB".into()],
                    partitions: vec![PartitionInfo {
                        name: "Macintosh HD".into(),
                        mount_point: Some("/".into()),
                        filesystem: Some("apfs".into()),
                        size_bytes: Some(900 * 1024 * 1024 * 1024),
                    }],
                    available_space_gb: Some(120),
                },
                boot_entries: vec![BootEntry {
                    id: "apple-default".into(),
                    label: "macOS".into(),
                    is_default: true,
                }],
                warnings: vec![
                    "Apple Silicon boot policy must not be bypassed.".into(),
                    "Use only Apple-supported installer mechanisms.".into(),
                ],
            },
            capabilities: vec![
                PlatformCapability::RecoveryPack,
                PlatformCapability::Snapshot,
                PlatformCapability::EncryptionStatus,
                PlatformCapability::SecureBootDetection,
            ],
        }
    }

    pub fn capabilities(&self) -> &[PlatformCapability] {
        &self.capabilities
    }
}

impl PlatformAdapter for FakePlatformAdapter {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError> {
        Ok(self.inventory.clone())
    }

    fn name(&self) -> &'static str {
        self.name
    }
}

pub fn execute_plan_safe(plan: &OperationPlan, yes: bool) -> Result<ExecutionOutcome, NoUsbError> {
    if yes && !plan.confirmation.permits_yes_flag(plan.risk) {
        return Err(NoUsbError::UnsafeExecution(format!(
            "--yes cannot approve {} risk plan {}; typed confirmation is required",
            plan.risk, plan.id
        )));
    }

    Err(NoUsbError::UnsafeExecution(format!(
        "execution of {} is disabled in the default non-destructive build",
        plan.id
    )))
}

#[derive(Debug)]
pub enum NoUsbError {
    Io(String),
    UnknownProfile(String),
    ProfileParse(String),
    UnsafeExecution(String),
    InvalidInput(String),
}

impl fmt::Display for NoUsbError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            NoUsbError::Io(value) => write!(f, "I/O error: {value}"),
            NoUsbError::UnknownProfile(value) => write!(f, "unknown installer profile: {value}"),
            NoUsbError::ProfileParse(value) => write!(f, "profile parse error: {value}"),
            NoUsbError::UnsafeExecution(value) => write!(f, "unsafe execution blocked: {value}"),
            NoUsbError::InvalidInput(value) => write!(f, "invalid input: {value}"),
        }
    }
}

impl std::error::Error for NoUsbError {}

impl From<io::Error> for NoUsbError {
    fn from(value: io::Error) -> Self {
        NoUsbError::Io(value.to_string())
    }
}

pub fn built_in_profiles() -> Vec<InstallerProfile> {
    match embedded_profiles() {
        Ok(profiles) => profiles,
        Err(error) => vec![InstallerProfile {
            schema_version: "1".into(),
            id: "profile-catalog-error".into(),
            name: "Profile catalog failed to load".into(),
            vendor: "NoUSB".into(),
            version: "error".into(),
            architecture: "unknown".into(),
            source_url: "about:blank".into(),
            checksum_url: "about:blank".into(),
            sha256: "unavailable".into(),
            signature_url: None,
            homepage: "about:blank".into(),
            license_notes: "Catalog parsing failed.".into(),
            supported_platforms: vec![],
            supported_staging_strategies: vec![StagingStrategy::ManualInstructions],
            required_free_space_gb: 1,
            extraction_rules: vec![],
            kernel_initrd_paths: vec![],
            bootloader_paths: vec![],
            secure_boot_notes: "Catalog parsing failed; do not use this placeholder.".into(),
            known_limitations: vec![error.to_string()],
            post_boot_instructions: "Fix the embedded profile catalog before use.".into(),
        }],
    }
}

pub fn embedded_profiles() -> Result<Vec<InstallerProfile>, NoUsbError> {
    let sources = [
        include_str!("../../nousb-installer-profiles/profiles/ubuntu-24.04-desktop.toml"),
        include_str!("../../nousb-installer-profiles/profiles/fedora-42-workstation.toml"),
        include_str!("../../nousb-installer-profiles/profiles/windows-11.toml"),
    ];
    sources
        .iter()
        .map(|source| parse_installer_profile(source))
        .collect()
}

pub fn parse_installer_profile(source: &str) -> Result<InstallerProfile, NoUsbError> {
    let fields = ProfileFields::parse(source)?;
    let profile = InstallerProfile {
        schema_version: fields.required("schema_version")?,
        id: fields.required("id")?,
        name: fields.required("name")?,
        vendor: fields.required("vendor")?,
        version: fields.required("version")?,
        architecture: fields.required("architecture")?,
        source_url: fields.required("source_url")?,
        checksum_url: fields.required("checksum_url")?,
        sha256: fields.required("sha256")?,
        signature_url: fields.optional("signature_url"),
        homepage: fields.required("homepage")?,
        license_notes: fields.required("license_notes")?,
        supported_platforms: fields.array("supported_platforms")?,
        supported_staging_strategies: fields
            .array("supported_staging_strategies")?
            .iter()
            .map(|value| StagingStrategy::from_profile_value(value))
            .collect::<Result<Vec<_>, _>>()?,
        required_free_space_gb: fields.required("required_free_space_gb")?.parse().map_err(
            |error| NoUsbError::ProfileParse(format!("required_free_space_gb is invalid: {error}")),
        )?,
        extraction_rules: fields.array("extraction_rules")?,
        kernel_initrd_paths: fields.array("kernel_initrd_paths")?,
        bootloader_paths: fields.array("bootloader_paths")?,
        secure_boot_notes: fields.required("secure_boot_notes")?,
        known_limitations: fields.array("known_limitations")?,
        post_boot_instructions: fields.required("post_boot_instructions")?,
    };
    validate_profile(&profile)?;
    Ok(profile)
}

pub fn validate_profile(profile: &InstallerProfile) -> Result<(), NoUsbError> {
    if profile.schema_version != "1" {
        return Err(NoUsbError::ProfileParse(format!(
            "{} uses unsupported schema_version {}",
            profile.id, profile.schema_version
        )));
    }
    if profile.id.trim().is_empty() {
        return Err(NoUsbError::ProfileParse("profile id is required".into()));
    }
    if profile.required_free_space_gb == 0 {
        return Err(NoUsbError::ProfileParse(format!(
            "{} must require nonzero free space",
            profile.id
        )));
    }
    if profile.supported_platforms.is_empty() {
        return Err(NoUsbError::ProfileParse(format!(
            "{} must declare supported platforms",
            profile.id
        )));
    }
    if profile.supported_staging_strategies.is_empty() {
        return Err(NoUsbError::ProfileParse(format!(
            "{} must declare staging strategies",
            profile.id
        )));
    }
    if profile.sha256.trim().is_empty() {
        return Err(NoUsbError::ProfileParse(format!(
            "{} must declare sha256 metadata",
            profile.id
        )));
    }
    Ok(())
}

pub fn find_profile(id: &str) -> Result<InstallerProfile, NoUsbError> {
    built_in_profiles()
        .into_iter()
        .find(|profile| profile.id == id)
        .ok_or_else(|| NoUsbError::UnknownProfile(id.into()))
}

pub fn plan_operation(
    kind: OperationKind,
    profile_id: Option<&str>,
) -> Result<OperationPlan, NoUsbError> {
    let profile = match profile_id {
        Some(id) => Some(find_profile(id)?),
        None => None,
    };
    let id = format!("plan-{}-{}", kind, now_stamp());
    let mut steps = vec![
        OperationStep {
            id: "inspect-system".into(),
            title: "Inspect current system".into(),
            description: "Collect firmware, encryption, disk, boot, and space metadata.".into(),
            risk: RiskLevel::Info,
            modifies: vec![],
            rollback: RollbackAction::None,
            state: StepState::Pending,
        },
        OperationStep {
            id: "create-recovery-pack".into(),
            title: "Create recovery pack".into(),
            description: "Write local metadata, boot inventory, and manual recovery notes.".into(),
            risk: RiskLevel::Low,
            modifies: vec!["./recovery-packs".into()],
            rollback: RollbackAction::RemoveStagedFiles,
            state: StepState::Pending,
        },
        OperationStep {
            id: "verify-installer".into(),
            title: "Verify installer artifact".into(),
            description: "Require SHA256 or stronger verification before execution.".into(),
            risk: RiskLevel::Medium,
            modifies: vec![],
            rollback: RollbackAction::None,
            state: StepState::Pending,
        },
    ];

    let (risk, confirmation, mut warnings) = match kind {
        OperationKind::BootOnce => (
            RiskLevel::Medium,
            ConfirmationRequirement::SimpleYes,
            vec!["Default boot target remains the current OS; real boot edits are disabled in this build.".into()],
        ),
        OperationKind::DualBoot => (
            RiskLevel::High,
            ConfirmationRequirement::TypePhrase("I understand dual-boot risk".into()),
            vec!["Partition changes are not performed by the default build.".into()],
        ),
        OperationKind::ReplaceOs => (
            RiskLevel::Critical,
            ConfirmationRequirement::TypePhrase("I have a verified full backup".into()),
            vec![
                "Replacing the current OS can destroy user data.".into(),
                "The default build will not wipe or repartition disks.".into(),
            ],
        ),
        OperationKind::ReinstallCurrentOs => (
            RiskLevel::High,
            ConfirmationRequirement::TypePhrase("I have reviewed reinstall risk".into()),
            vec!["Official installer flow must perform the final reinstall or upgrade.".into()],
        ),
        OperationKind::RecoveryPack => (RiskLevel::Low, ConfirmationRequirement::None, vec![]),
    };

    if let Some(profile) = &profile {
        warnings.push(format!(
            "Profile {} requires at least {} GB free space.",
            profile.id, profile.required_free_space_gb
        ));
    }

    steps.push(OperationStep {
        id: "stage-installer-dry-run".into(),
        title: "Preview installer staging".into(),
        description: "Show the staging strategy without modifying boot configuration.".into(),
        risk,
        modifies: vec!["no boot changes in default build".into()],
        rollback: RollbackAction::Manual(
            "Remove staged files if created by a future real adapter.".into(),
        ),
        state: StepState::Pending,
    });

    Ok(OperationPlan {
        id,
        boot_plan: Some(BootPlan::safe_default(&kind)),
        kind,
        profile_id: profile.map(|p| p.id),
        risk,
        required_privileges: vec![
            "administrator/root required only for real execution adapters".into(),
        ],
        confirmation,
        warnings,
        steps,
    })
}

pub fn plan_operation_for_inventory(
    kind: OperationKind,
    profile_id: Option<&str>,
    inventory: &SystemInventory,
) -> Result<OperationPlan, NoUsbError> {
    let profile = match profile_id {
        Some(id) => Some(find_profile(id)?),
        None => None,
    };
    let mut plan = plan_operation(kind, profile_id)?;

    for warning in &inventory.warnings {
        plan.warnings.push(format!("Preflight: {warning}"));
    }

    if let Some(profile) = &profile {
        match inventory.disk_inventory.available_space_gb {
            Some(available) if available < profile.required_free_space_gb => {
                plan.risk = plan.risk.max(RiskLevel::High);
                plan.warnings.push(format!(
                    "Preflight: only {available} GB free; profile {} requires {} GB.",
                    profile.id, profile.required_free_space_gb
                ));
                plan.steps.push(OperationStep {
                    id: "resolve-low-disk-space".into(),
                    title: "Resolve low disk space".into(),
                    description: "Free space or choose another staging location before proceeding."
                        .into(),
                    risk: RiskLevel::High,
                    modifies: vec![],
                    rollback: RollbackAction::Manual(
                        "No changes were made; free disk space and re-run planning.".into(),
                    ),
                    state: StepState::ManualInterventionRequired,
                });
            }
            None => plan
                .warnings
                .push("Preflight: available disk space could not be determined.".into()),
            _ => {}
        }
    }

    match &inventory.encryption {
        EncryptionState::Enabled(name) => {
            plan.warnings.push(format!(
                "Preflight: {name} encryption is enabled; recovery key/header backup confirmation is required before real boot changes."
            ));
        }
        EncryptionState::Unknown => plan
            .warnings
            .push("Preflight: encryption state is unknown.".into()),
        EncryptionState::Disabled => {}
    }

    if inventory.secure_boot == SecureBootState::Enabled {
        plan.warnings.push(
            "Preflight: Secure Boot is enabled; use only official signed boot assets and stop if firmware rejects them."
                .into(),
        );
    }

    Ok(plan)
}

pub fn write_journal_entry(base_dir: &Path, entry: &str) -> Result<PathBuf, NoUsbError> {
    let dir = base_dir.join(".nousb").join("journal");
    fs::create_dir_all(&dir)?;
    let sqlite_path = dir.join("operation_journal.sqlite3");
    let escaped_entry = entry.replace('\'', "''");
    let sql = format!(
        "CREATE TABLE IF NOT EXISTS operations (id INTEGER PRIMARY KEY AUTOINCREMENT, created_at INTEGER NOT NULL, message TEXT NOT NULL);
         INSERT INTO operations (created_at, message) VALUES ({}, '{}');",
        now_stamp(),
        escaped_entry
    );
    let sqlite_result = std::process::Command::new("sqlite3")
        .arg(&sqlite_path)
        .arg(sql)
        .output();

    let log_path = dir.join("operations.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&log_path)?;
    writeln!(file, "{} {entry}", now_stamp())?;
    match sqlite_result {
        Ok(output) if output.status.success() => Ok(sqlite_path),
        Ok(_) => Ok(log_path),
        Err(_) => Ok(log_path),
    }
}

pub fn create_recovery_pack(
    base_dir: &Path,
    inventory: &SystemInventory,
) -> Result<PathBuf, NoUsbError> {
    let dir = base_dir
        .join("recovery-packs")
        .join(format!("pack-{}", now_stamp()));
    fs::create_dir_all(&dir)?;
    let mut file = File::create(dir.join("RECOVERY.md"))?;
    writeln!(file, "# NoUSB Recovery Pack")?;
    writeln!(file)?;
    writeln!(
        file,
        "This pack contains metadata only. It is not a full user-data backup."
    )?;
    writeln!(file)?;
    writeln!(file, "## System")?;
    writeln!(file, "- OS: {}", inventory.os_name)?;
    writeln!(file, "- Architecture: {}", inventory.arch)?;
    writeln!(file, "- Firmware: {:?}", inventory.firmware)?;
    writeln!(file, "- Secure Boot: {:?}", inventory.secure_boot)?;
    writeln!(file, "- Encryption: {:?}", inventory.encryption)?;
    writeln!(file)?;
    writeln!(file, "## Manual Recovery")?;
    writeln!(
        file,
        "1. Reboot and choose the original OS entry from firmware boot menu."
    )?;
    writeln!(file, "2. If boot metadata was changed by a future real adapter, restore the recorded boot backup.")?;
    writeln!(
        file,
        "3. Use vendor recovery media if the original OS no longer boots."
    )?;
    Ok(dir)
}

pub fn verify_sha256_with_system_tool(path: &Path, expected: &str) -> Result<bool, NoUsbError> {
    if expected.len() != 64 || !expected.chars().all(|c| c.is_ascii_hexdigit()) {
        return Err(NoUsbError::InvalidInput(
            "expected SHA256 must be 64 hex characters".into(),
        ));
    }
    let output = std::process::Command::new("sha256sum").arg(path).output()?;
    if !output.status.success() {
        return Err(NoUsbError::Io(
            String::from_utf8_lossy(&output.stderr).to_string(),
        ));
    }
    let stdout = String::from_utf8_lossy(&output.stdout);
    let actual = stdout.split_whitespace().next().unwrap_or_default();
    Ok(actual.eq_ignore_ascii_case(expected))
}

pub struct HostPlatform;

impl PlatformAdapter for HostPlatform {
    fn inspect(&self) -> Result<SystemInventory, NoUsbError> {
        let os_name = std::env::consts::OS.to_string();
        let arch = std::env::consts::ARCH.to_string();
        let firmware = if Path::new("/sys/firmware/efi").exists() {
            FirmwareMode::Uefi
        } else {
            FirmwareMode::Unknown
        };
        let secure_boot = if Path::new("/sys/firmware/efi/efivars").exists() {
            SecureBootState::Unknown
        } else {
            SecureBootState::Unsupported
        };
        let encryption = if Path::new("/etc/crypttab").exists() {
            EncryptionState::Unknown
        } else {
            EncryptionState::Unknown
        };
        let mut warnings =
            vec!["Default build performs inspection only and will not edit boot settings.".into()];
        if os_name == "macos" {
            warnings.push("Apple Silicon boot policy must not be bypassed.".into());
        }
        Ok(SystemInventory {
            os_name,
            arch,
            firmware,
            secure_boot,
            encryption,
            disk_inventory: DiskInventory {
                disks: vec!["host disk inventory requires privileged platform adapter".into()],
                partitions: vec![],
                available_space_gb: None,
            },
            boot_entries: vec![],
            warnings,
        })
    }

    fn name(&self) -> &'static str {
        "host-safe"
    }
}

pub fn read_to_bytes(path: &Path) -> Result<Vec<u8>, NoUsbError> {
    let mut file = File::open(path)?;
    let mut bytes = Vec::new();
    file.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn now_stamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs())
        .unwrap_or(0)
}

#[derive(Debug)]
struct ProfileFields {
    values: Vec<(String, ProfileValue)>,
}

#[derive(Debug)]
enum ProfileValue {
    String(String),
    Array(Vec<String>),
}

impl ProfileFields {
    fn parse(source: &str) -> Result<Self, NoUsbError> {
        let mut values = Vec::new();
        for (index, raw_line) in source.lines().enumerate() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((raw_key, raw_value)) = line.split_once('=') else {
                return Err(NoUsbError::ProfileParse(format!(
                    "line {} is missing '='",
                    index + 1
                )));
            };
            let key = raw_key.trim().to_string();
            let value = parse_profile_value(raw_value.trim()).map_err(|error| {
                NoUsbError::ProfileParse(format!("line {} for key {key}: {error}", index + 1))
            })?;
            values.push((key, value));
        }
        Ok(Self { values })
    }

    fn required(&self, key: &str) -> Result<String, NoUsbError> {
        match self.value(key) {
            Some(ProfileValue::String(value)) => Ok(value.clone()),
            Some(ProfileValue::Array(_)) => Err(NoUsbError::ProfileParse(format!(
                "{key} must be a string value"
            ))),
            None => Err(NoUsbError::ProfileParse(format!(
                "missing required field {key}"
            ))),
        }
    }

    fn optional(&self, key: &str) -> Option<String> {
        match self.value(key) {
            Some(ProfileValue::String(value)) if !value.is_empty() => Some(value.clone()),
            _ => None,
        }
    }

    fn array(&self, key: &str) -> Result<Vec<String>, NoUsbError> {
        match self.value(key) {
            Some(ProfileValue::Array(value)) => Ok(value.clone()),
            Some(ProfileValue::String(_)) => Err(NoUsbError::ProfileParse(format!(
                "{key} must be an array value"
            ))),
            None => Err(NoUsbError::ProfileParse(format!(
                "missing required field {key}"
            ))),
        }
    }

    fn value(&self, key: &str) -> Option<&ProfileValue> {
        self.values
            .iter()
            .find_map(|(candidate, value)| (candidate == key).then_some(value))
    }
}

fn parse_profile_value(source: &str) -> Result<ProfileValue, String> {
    if source.starts_with('"') {
        parse_quoted_string(source).map(ProfileValue::String)
    } else if source.starts_with('[') {
        parse_string_array(source).map(ProfileValue::Array)
    } else {
        Ok(ProfileValue::String(source.to_string()))
    }
}

fn parse_quoted_string(source: &str) -> Result<String, String> {
    let trimmed = source.trim();
    if !trimmed.starts_with('"') || !trimmed.ends_with('"') {
        return Err("quoted string must start and end with double quotes".into());
    }
    Ok(unescape_profile_string(&trimmed[1..trimmed.len() - 1]))
}

fn parse_string_array(source: &str) -> Result<Vec<String>, String> {
    let trimmed = source.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err("array must start with '[' and end with ']'".into());
    }
    let inner = trimmed[1..trimmed.len() - 1].trim();
    if inner.is_empty() {
        return Ok(vec![]);
    }

    let mut values = Vec::new();
    let mut current = String::new();
    let mut in_string = false;
    let mut escaped = false;

    for character in inner.chars() {
        if escaped {
            current.push(character);
            escaped = false;
            continue;
        }
        match character {
            '\\' if in_string => escaped = true,
            '"' => {
                in_string = !in_string;
                current.push(character);
            }
            ',' if !in_string => {
                values.push(parse_quoted_string(current.trim())?);
                current.clear();
            }
            _ => current.push(character),
        }
    }

    if in_string {
        return Err("unterminated string in array".into());
    }
    if !current.trim().is_empty() {
        values.push(parse_quoted_string(current.trim())?);
    }
    Ok(values)
}

fn unescape_profile_string(source: &str) -> String {
    let mut output = String::new();
    let mut escaped = false;
    for character in source.chars() {
        if escaped {
            match character {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else {
            output.push(character);
        }
    }
    output
}

fn json_pair(key: &str, value: &str) -> String {
    format!("\"{}\":\"{}\"", escape_json(key), escape_json(value))
}

fn json_optional_pair(key: &str, value: Option<&str>) -> String {
    match value {
        Some(value) => json_pair(key, value),
        None => format!("\"{}\":null", escape_json(key)),
    }
}

fn json_optional_value(value: Option<&str>) -> String {
    match value {
        Some(value) => format!("\"{}\"", escape_json(value)),
        None => "null".into(),
    }
}

fn json_string_array(values: &[String]) -> String {
    let values: Vec<String> = values
        .iter()
        .map(|value| format!("\"{}\"", escape_json(value)))
        .collect();
    format!("[{}]", values.join(","))
}

fn json_field(source: &str, key: &str) -> Option<String> {
    let marker = format!("\"{}\":\"", escape_json(key));
    let start = source.find(&marker)? + marker.len();
    let mut output = String::new();
    let mut escaped = false;
    for character in source[start..].chars() {
        if escaped {
            match character {
                'n' => output.push('\n'),
                'r' => output.push('\r'),
                't' => output.push('\t'),
                '"' => output.push('"'),
                '\\' => output.push('\\'),
                other => output.push(other),
            }
            escaped = false;
        } else if character == '\\' {
            escaped = true;
        } else if character == '"' {
            return Some(output);
        } else {
            output.push(character);
        }
    }
    None
}

fn json_bool_field(source: &str, key: &str) -> Option<bool> {
    let marker = format!("\"{}\":", escape_json(key));
    let start = source.find(&marker)? + marker.len();
    let remaining = source[start..].trim_start();
    if remaining.starts_with("true") {
        Some(true)
    } else if remaining.starts_with("false") {
        Some(false)
    } else {
        None
    }
}

fn safe_filename(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect()
}

fn escape_json(value: &str) -> String {
    let mut escaped = String::new();
    for character in value.chars() {
        match character {
            '"' => escaped.push_str("\\\""),
            '\\' => escaped.push_str("\\\\"),
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            character if character.is_control() => {
                escaped.push_str(&format!("\\u{:04x}", character as u32));
            }
            character => escaped.push(character),
        }
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replace_os_is_critical_and_typed() {
        let plan = plan_operation(OperationKind::ReplaceOs, Some("ubuntu-24.04-desktop")).unwrap();
        assert_eq!(plan.risk, RiskLevel::Critical);
        assert!(matches!(
            plan.confirmation,
            ConfirmationRequirement::TypePhrase(_)
        ));
    }

    #[test]
    fn unknown_profile_fails() {
        let result = plan_operation(OperationKind::BootOnce, Some("missing"));
        assert!(matches!(result, Err(NoUsbError::UnknownProfile(_))));
    }

    #[test]
    fn profile_catalog_has_safe_defaults() {
        let profiles = built_in_profiles();
        assert!(profiles
            .iter()
            .any(|profile| profile.id == "ubuntu-24.04-desktop"));
        assert!(profiles
            .iter()
            .all(|profile| profile.required_free_space_gb > 0));
    }

    #[test]
    fn embedded_profiles_parse_and_validate() {
        let profiles = embedded_profiles().unwrap();
        assert_eq!(profiles.len(), 3);
        assert!(profiles.iter().all(|profile| profile.schema_version == "1"));
        assert!(profiles
            .iter()
            .any(|profile| profile.supported_platforms.contains(&"linux_uefi".into())));
    }

    #[test]
    fn profile_validation_rejects_empty_platforms() {
        let source = r#"
schema_version = "1"
id = "bad"
name = "Bad"
vendor = "NoUSB"
version = "0"
architecture = "x86_64"
source_url = "https://example.invalid"
checksum_url = "https://example.invalid/SHA256SUMS"
sha256 = "placeholder"
signature_url = ""
homepage = "https://example.invalid"
license_notes = "None"
supported_platforms = []
supported_staging_strategies = ["manual_instructions"]
required_free_space_gb = 1
extraction_rules = []
kernel_initrd_paths = []
bootloader_paths = []
secure_boot_notes = "None"
known_limitations = []
post_boot_instructions = "None"
"#;
        let result = parse_installer_profile(source);
        assert!(matches!(result, Err(NoUsbError::ProfileParse(_))));
    }

    #[test]
    fn plan_json_contains_core_fields() {
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let json = plan.to_json();
        assert!(json.contains("\"kind\":\"boot-once\""));
        assert!(json.contains("\"risk\":\"medium\""));
        assert!(json.contains("\"boot_plan\""));
        assert!(json.contains("\"steps\""));
    }

    #[test]
    fn boot_once_plan_prefers_one_time_boot() {
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let boot_plan = plan.boot_plan.unwrap();
        assert_eq!(
            boot_plan.selected_preference,
            BootPlanPreference::OneTimeBoot
        );
        assert!(boot_plan.keep_current_os_default);
        assert!(!boot_plan.allow_permanent_boot_order_change);
    }

    #[test]
    fn replace_os_plan_still_disallows_permanent_boot_change_by_default() {
        let plan = plan_operation(OperationKind::ReplaceOs, Some("ubuntu-24.04-desktop")).unwrap();
        let boot_plan = plan.boot_plan.unwrap();
        assert_eq!(
            boot_plan.selected_preference,
            BootPlanPreference::TemporaryBootEntryWithFallback
        );
        assert!(boot_plan.keep_current_os_default);
        assert!(!boot_plan.allow_permanent_boot_order_change);
    }

    #[test]
    fn yes_flag_cannot_approve_critical_plan() {
        let plan = plan_operation(OperationKind::ReplaceOs, Some("ubuntu-24.04-desktop")).unwrap();
        let result = execute_plan_safe(&plan, true);
        assert!(matches!(result, Err(NoUsbError::UnsafeExecution(_))));
    }

    #[test]
    fn fake_windows_bitlocker_adds_preflight_warnings() {
        let adapter = FakePlatformAdapter::windows_uefi_bitlocker();
        let inventory = adapter.inspect().unwrap();
        let plan = plan_operation_for_inventory(
            OperationKind::BootOnce,
            Some("ubuntu-24.04-desktop"),
            &inventory,
        )
        .unwrap();

        assert_eq!(inventory.secure_boot, SecureBootState::Enabled);
        assert!(adapter
            .capabilities()
            .contains(&PlatformCapability::BootEntryBackup));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("BitLocker")));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("Secure Boot")));
    }

    #[test]
    fn fake_linux_low_space_requires_manual_intervention() {
        let inventory = FakePlatformAdapter::linux_grub_luks_btrfs_low_space()
            .inspect()
            .unwrap();
        let plan = plan_operation_for_inventory(
            OperationKind::BootOnce,
            Some("ubuntu-24.04-desktop"),
            &inventory,
        )
        .unwrap();

        assert_eq!(plan.risk, RiskLevel::High);
        assert!(plan
            .steps
            .iter()
            .any(|step| step.id == "resolve-low-disk-space"
                && step.state == StepState::ManualInterventionRequired));
        assert!(plan.warnings.iter().any(|warning| warning.contains("LUKS")));
    }

    #[test]
    fn fake_macos_apple_silicon_warns_about_policy() {
        let inventory = FakePlatformAdapter::macos_apple_silicon_filevault()
            .inspect()
            .unwrap();
        let plan = plan_operation_for_inventory(
            OperationKind::ReinstallCurrentOs,
            Some("windows-11"),
            &inventory,
        )
        .unwrap();

        assert_eq!(inventory.arch, "aarch64");
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("Apple Silicon boot policy")));
        assert!(plan
            .warnings
            .iter()
            .any(|warning| warning.contains("FileVault")));
    }

    #[test]
    fn simulated_executor_can_complete_safe_plan() {
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let report = SimulatedExecutor::new(SimulationConfig::complete())
            .execute(&plan)
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Completed);
        assert!(!report.recovery_required);
        assert!(report
            .steps
            .iter()
            .all(|step| step.state == StepState::Completed));
    }

    #[test]
    fn simulated_executor_can_interrupt_after_completed_step() {
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let report = SimulatedExecutor::new(SimulationConfig::interrupt_after(2))
            .execute(&plan)
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Interrupted);
        assert!(report.recovery_required);
        assert_eq!(report.steps[0].state, StepState::Completed);
        assert_eq!(report.steps[1].state, StepState::Completed);
        assert_eq!(report.steps[2].state, StepState::Pending);
    }

    #[test]
    fn simulated_executor_failure_marks_step_failed() {
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let report = SimulatedExecutor::new(SimulationConfig::fail_step("verify-installer"))
            .execute(&plan)
            .unwrap();

        assert_eq!(report.status, ExecutionStatus::Failed);
        assert!(report.recovery_required);
        assert!(report
            .steps
            .iter()
            .any(|step| step.id == "verify-installer" && step.state == StepState::Failed));
    }

    #[test]
    fn simulated_rollback_rolls_back_completed_steps_with_actions() {
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let interrupted = SimulatedExecutor::new(SimulationConfig::interrupt_after(2))
            .execute(&plan)
            .unwrap();
        let rolled_back = rollback_simulated_report(&interrupted);

        assert_eq!(rolled_back.status, ExecutionStatus::RolledBack);
        assert!(rolled_back
            .steps
            .iter()
            .any(|step| step.id == "create-recovery-pack" && step.state == StepState::RolledBack));
        assert!(rolled_back
            .steps
            .iter()
            .any(|step| step.id == "inspect-system" && step.state == StepState::Completed));
    }

    #[test]
    fn execution_reports_can_be_saved_listed_and_loaded() {
        let base_dir = std::env::temp_dir().join(format!("nousb-test-{}", now_stamp()));
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let report = SimulatedExecutor::new(SimulationConfig::interrupt_after(1))
            .execute(&plan)
            .unwrap();

        let path = save_execution_report(&base_dir, &report).unwrap();
        assert!(path.exists());

        let summaries = list_saved_execution_reports(&base_dir).unwrap();
        assert_eq!(summaries.len(), 1);
        assert_eq!(summaries[0].plan_id, report.plan_id);
        assert_eq!(summaries[0].status, "interrupted");
        assert!(summaries[0].recovery_required);

        let loaded = load_execution_report_json(&base_dir, &report.plan_id).unwrap();
        assert!(loaded.contains("\"status\":\"interrupted\""));

        let _ = fs::remove_dir_all(base_dir);
    }

    #[test]
    fn diagnostic_bundle_redacts_secret_like_lines() {
        let redacted =
            redact_secrets("password=hunter2\nsafe=value\nBitLocker recovery_key: 12345");
        assert!(redacted.contains("password=[REDACTED]"));
        assert!(redacted.contains("safe=value"));
        assert!(redacted.contains("BitLocker recovery_key: [REDACTED]"));
        assert!(!redacted.contains("hunter2"));
        assert!(!redacted.contains("12345"));
    }

    #[test]
    fn diagnostic_bundle_exports_project_local_artifacts() {
        let base_dir = std::env::temp_dir().join(format!("nousb-diag-test-{}", now_stamp()));
        let plan = plan_operation(OperationKind::BootOnce, Some("ubuntu-24.04-desktop")).unwrap();
        let report = SimulatedExecutor::new(SimulationConfig::interrupt_after(1))
            .execute(&plan)
            .unwrap();
        save_execution_report(&base_dir, &report).unwrap();
        write_journal_entry(&base_dir, "token=super-secret").unwrap();

        let inventory = FakePlatformAdapter::windows_uefi_bitlocker()
            .inspect()
            .unwrap();
        let bundle = export_diagnostic_bundle(&base_dir, &inventory).unwrap();
        assert!(bundle.path.exists());
        assert!(bundle
            .files
            .iter()
            .any(|path| path.ends_with("system-inventory.json")));
        assert!(bundle
            .files
            .iter()
            .any(|path| path.ends_with("execution-report-index.json")));

        let log = fs::read_to_string(bundle.path.join("operations.log")).unwrap();
        assert!(log.contains("token=[REDACTED]"));
        assert!(!log.contains("super-secret"));

        let _ = fs::remove_dir_all(base_dir);
    }
}
