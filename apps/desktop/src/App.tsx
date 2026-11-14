import React, { useCallback, useEffect, useState } from "react";
import { createRoot } from "react-dom/client";
import {
  bridge,
  CLEANUP_PHRASE,
  CUSTOM_PROFILE,
  WINDOWS_PROFILE,
  downloadPhrase,
  isLive,
  type BootSource,
  type HostBootPlan,
  type ImageStatus,
  type HostBootState,
  type Inventory,
  type Profile,
  type SavedReport,
  type SignatureVerification,
  type VmEvidence,
} from "./bridge";
import "./style.css";

const views = ["overview", "boot once", "delivery", "vm evidence", "catalog", "planner", "reports", "diagnostics"] as const;
type View = (typeof views)[number];

const commands = [
  "cargo run -p nousb -- tui",
  "cargo run -p nousb -- system inspect",
  "cargo run -p nousb -- vm doctor",
  "cargo run -p nousb -- vm qemu bootstrap-runtime \"ASSEMBLE PROJECT-LOCAL QEMU RUNTIME\"",
  "cargo run -p nousb -- vm qemu rehearse-current-os-one-shot \"RUN DISPOSABLE CURRENT OS UEFI ONE-SHOT MATRIX\"",
  "cargo run -p nousb -- vm qemu rehearse-current-os-one-shot \"RUN DISPOSABLE CURRENT OS UEFI ONE-SHOT MATRIX\" linux-grub-reboot",
  "cargo run -p nousb -- vm qemu rehearse-secure-boot \"RUN DISPOSABLE SECURE BOOT REHEARSAL\"",
  "cargo run -p nousb -- image verify-signature ubuntu-24.04-server",
  "cargo run -p nousb -- image status ubuntu-24.04-desktop",
  "cargo run -p nousb -- boot plan ubuntu-24.04-desktop",
  "cargo run -p nousb -- boot plan custom --iso <linux-live.iso> --sha256 <hex>",
  "cargo run -p nousb -- boot plan windows-11 --iso <windows.iso>",
  "cargo run -p nousb -- --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop",
  "cargo run -p nousb -- reports recover <plan-id>",
  "cargo run -p nousb -- journal migrate",
];

/** Loads one bridge call, keeping the error so the view can say why it shows preview text. */
function useBridge<T>(load: () => Promise<T>, deps: unknown[] = []) {
  const [data, setData] = useState<T | null>(null);
  const [error, setError] = useState<string | null>(null);
  const refresh = useCallback(() => {
    load()
      .then((value) => {
        setData(value);
        setError(null);
      })
      .catch((reason: unknown) => setError(String(reason instanceof Error ? reason.message : reason)));
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, deps);
  useEffect(refresh, [refresh]);
  return { data, error, refresh };
}

function App() {
  const [view, setView] = useState<View>("overview");
  const live = isLive();

  return (
    <main>
      <header>
        <div>
          <p className="eyebrow">NoUSB OS Switcher</p>
          <h1>Safe installer staging, preflight planning, and recovery rehearsal</h1>
        </div>
        <div className="statusbar" aria-label="Safety status">
          <span className="pill">Boot changes need a real-boot build + typed phrase</span>
          <span className="pill">{live ? "Live: desktop bridge" : "Preview: static data"}</span>
          <span className="pill">Diagnostics redacted</span>
        </div>
      </header>

      <div className="layout">
        <nav aria-label="Views">
          {views.map((item, index) => (
            <button
              className="navbutton"
              key={item}
              aria-selected={view === item}
              onClick={() => setView(item)}
            >
              <span>{label(item)}</span>
              <span>{index + 1}</span>
            </button>
          ))}
        </nav>

        <div className="content">
          {view === "overview" && <Overview />}
          {view === "boot once" && <BootOnce />}
          {view === "delivery" && <Delivery />}
          {view === "vm evidence" && <Evidence />}
          {view === "catalog" && <Catalog />}
          {view === "planner" && <Planner />}
          {view === "reports" && <Reports />}
          {view === "diagnostics" && <Diagnostics />}
        </div>
      </div>
    </main>
  );
}

function Overview() {
  const { data, error } = useBridge<Inventory>(bridge.inventory);
  if (!data) {
    return (
      <section>
        <SectionHead title="System Summary" detail={error ?? "Inspecting the host read-only..."} />
        <div className="grid">
          <Panel title="Guided TUI" text="Run nousb tui for profile selection, feasibility checks, verified acquisition, staging, and plan previews." />
          <Panel title="Read-only inspection" text="Firmware, Secure Boot, encryption, ESP, boot managers, and firmware boot entries; unknowns stay explicit." />
          <Panel title="Local State" text="Verified images, staging bundles, reports, the SQLite journal, and disposable VM evidence stay under .nousb/." />
        </div>
      </section>
    );
  }
  const hints = data.boot_hints;
  return (
    <section>
      <SectionHead title="System Summary" detail={`Collected read-only by ${data.adapter}.`} />
      <table>
        <tbody>
          <Row name="OS" value={data.os_name} />
          <Row name="Firmware" value={data.firmware} />
          <Row name="Secure Boot" value={data.secure_boot} />
          <Row name="Encryption" value={data.encryption} />
          <Row name="Memory" value={data.memory_total_mb === null ? "unknown" : `${data.memory_total_mb} MiB`} />
          <Row name="ESP" value={tri(hints.esp_present) + (hints.esp_location ? ` (${hints.esp_location})` : "")} />
          <Row name="Boot managers" value={hints.boot_managers.join(", ") || "unknown"} />
          <Row name="Firmware BootOrder" value={hints.firmware_boot_order?.join(", ") ?? "unknown"} />
          <Row
            name="Network boot entries"
            value={hints.network_boot_entries === null ? "unknown" : `${hints.network_boot_entries} (HTTP: ${hints.http_boot_entries})`}
          />
        </tbody>
      </table>
      {hints.unknowns.length > 0 && (
        <ul className="steps">
          {hints.unknowns.map((unknown) => (
            <li key={unknown}><strong>unknown</strong><span>{unknown}</span></li>
          ))}
        </ul>
      )}
    </section>
  );
}

/** Returns `value` once it has stopped changing for `delay` ms, so typing does not re-plan per key. */
function useDebounced<T>(value: T, delay = 500): T {
  const [settled, setSettled] = useState(value);
  useEffect(() => {
    const timer = setTimeout(() => setSettled(value), delay);
    return () => clearTimeout(timer);
  }, [value, delay]);
  return settled;
}

const mib = (bytes: number): string => `${Math.round(bytes / (1024 * 1024))} MiB`;

/** Download state of a pinned image, with the typed-phrase download control. */
function ImagePanel({ profile, onChanged }: { profile: string; onChanged: () => void }) {
  const status = useBridge<ImageStatus>(() => bridge.imageStatus(profile), [profile]);
  const [phrase, setPhrase] = useState("");
  const [busy, setBusy] = useState(false);
  const [message, setMessage] = useState<string | null>(null);
  const active = busy || Boolean(status.data?.downloading);
  useEffect(() => {
    if (!active) return;
    const timer = setInterval(status.refresh, 2000);
    return () => clearInterval(timer);
  }, [active, status.refresh]);
  useEffect(() => {
    setPhrase("");
    setMessage(null);
  }, [profile]);
  const data = status.data;
  if (!data) {
    return <p className="muted">{status.error ?? "Checking the local image..."}</p>;
  }
  const start = () => {
    setBusy(true);
    setMessage(null);
    bridge
      .imageDownload(profile, phrase)
      .then(() => setMessage("Downloaded and verified against the pinned size and SHA-256."))
      .catch((reason: unknown) => setMessage(String(reason)))
      .finally(() => {
        setBusy(false);
        setPhrase("");
        status.refresh();
        onChanged();
      });
  };
  const state = data.present
    ? "downloaded and verified"
    : active
      ? `downloading: ${mib(data.partial_bytes)} of ${mib(data.size_bytes)}`
      : data.partial_bytes > 0
        ? `partial: ${mib(data.partial_bytes)} of ${mib(data.size_bytes)} (download resumes)`
        : "not downloaded";
  return (
    <>
      <table>
        <tbody>
          <Row name="Official image" value={`${state} (${mib(data.size_bytes)})`} />
          <Row name="Location" value={data.path} />
        </tbody>
      </table>
      {message && <p className="muted">{message}</p>}
      {!data.present && (
        <p>
          <label>
            Type <code>{downloadPhrase(profile)}</code> to download{" "}
            <input value={phrase} disabled={active} onChange={(event) => setPhrase(event.target.value)} />
          </label>{" "}
          <button className="copy" disabled={!isLive() || active || phrase !== downloadPhrase(profile)} onClick={start}>
            {active ? "Downloading..." : data.partial_bytes > 0 ? "Resume download" : "Download and verify"}
          </button>
        </p>
      )}
    </>
  );
}

function BootOnce() {
  const [profile, setProfile] = useState("ubuntu-24.04-desktop");
  const [stagingRoot, setStagingRoot] = useState("");
  const [iso, setIso] = useState("");
  const [sha256, setSha256] = useState("");
  const [phrase, setPhrase] = useState("");
  const [message, setMessage] = useState<string | null>(null);
  const custom = profile === CUSTOM_PROFILE;
  const windows = profile === WINDOWS_PROFILE;
  const typed = useDebounced(`${stagingRoot.trim()}\n${iso.trim()}\n${custom ? sha256.trim() : ""}`);
  const [root, isoPath, digest] = typed.split("\n");
  const source: BootSource = { stagingRoot: root || undefined, iso: isoPath || undefined, sha256: digest || undefined };
  const plan = useBridge<HostBootPlan>(() => bridge.hostBootPlan(profile, source), [profile, typed]);
  const status = useBridge<HostBootState | null>(bridge.hostBootStatus);
  const run = (action: Promise<HostBootState>, done: string) => {
    setMessage("Working... copying and hashing a multi-gigabyte image takes several minutes.");
    action
      .then((state) => {
        setMessage(`${done} (${state.phase})`);
        setPhrase("");
        status.refresh();
        plan.refresh();
      })
      .catch((reason: unknown) => setMessage(String(reason)));
  };
  const data = plan.data;
  const pending = status.data && ["preparing", "armed", "failed"].includes(status.data.phase);
  const armable = data?.target_kind === "linux_live";
  return (
    <section>
      <SectionHead
        title="Boot Once Into an Installer"
        detail={message ?? plan.error ?? "One firmware boot into a live installer; every later boot returns to this OS."}
        risk={data ? (data.blockers.length === 0 ? "ready" : `${data.blockers.length} blocker(s)`) : undefined}
      />
      <ProfilePicker value={profile} onChange={setProfile} extra={[CUSTOM_PROFILE]} />
      {!custom && !windows && <ImagePanel profile={profile} onChanged={plan.refresh} />}
      <p>
        <label>
          {custom
            ? "Linux live ISO on this PC (required)"
            : windows
              ? "Windows ISO on this PC (you download it from Microsoft)"
              : "Use an ISO you already have instead (optional)"}{" "}
          <input size={48} value={iso} placeholder={custom || windows ? "full path to the .iso" : "default: the downloaded copy"} onChange={(event) => setIso(event.target.value)} />
        </label>
      </p>
      {custom && (
        <p>
          <label>
            Expected SHA-256 from the vendor (recommended){" "}
            <input size={48} value={sha256} placeholder="64 hex digits" onChange={(event) => setSha256(event.target.value)} />
          </label>
        </p>
      )}
      <p>
        <label>
          Staging volume root{" "}
          <input value={stagingRoot} placeholder="default: system volume" onChange={(event) => setStagingRoot(event.target.value)} />
        </label>
      </p>
      {status.data && (
        <table>
          <tbody>
            <Row name="Recorded transaction" value={status.data.plan_id} />
            <Row name="Phase" value={status.data.phase} />
            <Row name="Firmware entry" value={status.data.entry_id ?? "none"} />
          </tbody>
        </table>
      )}
      {data && (
        <>
          <table>
            <tbody>
              <Row name="Target" value={data.profile_name} />
              <Row name="ISO" value={data.iso_path || "none"} />
              <Row name="ISO identity" value={data.iso_verification.replace("_", " ")} />
              <Row name="Real boot backend in this build" value={data.execution_enabled ? "yes" : "no (safe build)"} />
            </tbody>
          </table>
          <ul className="steps">
            {data.blockers.map((item) => (
              <li key={item}><strong>blocker</strong><span>{item}</span></li>
            ))}
            {data.warnings.map((item) => (
              <li key={item}><strong>warning</strong><span>{item}</span></li>
            ))}
          </ul>
          <h3>Steps</h3>
          <ol>{data.steps.map((step) => <li key={step}>{step}</li>)}</ol>
          {armable && (
            <>
              <h3>Files created (all removed by cleanup)</h3>
              <ul>{data.files.map((file) => <li key={file.location + file.relative_path}>[{file.location}] {file.relative_path}</li>)}</ul>
              <h3>Generated GRUB configuration</h3>
              <pre>{data.grub_config}</pre>
            </>
          )}
          <h3>If anything goes wrong</h3>
          <ul>{data.recovery.map((line) => <li key={line}>{line}</li>)}</ul>
          {(pending || armable) && (
            <p>
              <label>
                Type <code>{pending ? CLEANUP_PHRASE : data.confirmation_phrase}</code> to confirm{" "}
                <input value={phrase} onChange={(event) => setPhrase(event.target.value)} />
              </label>
            </p>
          )}
          {pending ? (
            <button className="copy" disabled={!isLive() || phrase !== CLEANUP_PHRASE} onClick={() => run(bridge.hostBootCleanup(phrase), "Cleanup finished")}>
              Clean up
            </button>
          ) : armable ? (
            <button
              className="copy"
              disabled={!isLive() || !data.execution_enabled || data.blockers.length > 0 || phrase !== data.confirmation_phrase}
              onClick={() => run(bridge.hostBootArm(profile, phrase, source), "Armed: restart to boot the installer once")}
            >
              Arm one-shot boot
            </button>
          ) : (
            <p className="muted">This target is plan-only. Nothing can be armed for it.</p>
          )}
        </>
      )}
    </section>
  );
}

function Delivery() {
  const [profile, setProfile] = useState("ubuntu-24.04-server");
  const { data, error } = useBridge<Record<string, unknown>>(() => bridge.delivery(profile), [profile]);
  return (
    <section>
      <SectionHead title="No-USB Delivery" detail="Feasibility for this host before planning or execution." />
      <ProfilePicker value={profile} onChange={setProfile} />
      {data ? <pre>{JSON.stringify(data, null, 2)}</pre> : (
        <div className="grid">
          <Panel title="Recommended design" text="Acquire the pinned official ISO, extract only declared Ubuntu boot assets, then copy live media to RAM after boot begins." />
          <Panel title="Firmware alternative" text="UEFI HTTP Boot avoids disk staging only when the firmware and preboot network device support it." />
          <Panel title="Bridge" text={error ?? "Loading..."} />
        </div>
      )}
    </section>
  );
}

function Evidence() {
  const { data, error } = useBridge<VmEvidence[]>(bridge.vmEvidence);
  return (
    <section>
      <SectionHead title="Disposable VM Evidence" detail="Real QEMU runs, kept separate from host boot execution." />
      {data && data.length > 0 ? (
        <table>
          <thead><tr><th>Run</th><th>Key results</th></tr></thead>
          <tbody>
            {data.map((item) => (
              <tr key={item.run}>
                <td>{item.run}</td>
                <td>{keyResults(item.report)}</td>
              </tr>
            ))}
          </tbody>
        </table>
      ) : (
        <div className="grid">
          <Panel title="UEFI BootNext matrix" text="A disposable current OS arms BootNext with efibootmgr; the official installer chain runs exactly once and firmware falls back to the unchanged default after power loss at every step." />
          <Panel title="GRUB next_entry matrix" text="The same six-scenario power-loss matrix passes with grub-reboot semantics on the current OS's own GRUB." />
          <Panel title="Full installer in RAM" text="The pinned 3.2 GiB Server ISO continues from RAM after its EFI and HTTP sources are removed." />
          <Panel title="Bridge" text={error ?? "No saved VM evidence in this project yet."} />
        </div>
      )}
    </section>
  );
}

function Catalog() {
  const { data, error } = useBridge<Profile[]>(bridge.catalog);
  const [signatures, setSignatures] = useState<Record<string, string>>({});
  const verify = (profile: string) => {
    setSignatures((current) => ({ ...current, [profile]: "checking..." }));
    bridge
      .verifySignature(profile)
      .then((result: SignatureVerification) =>
        setSignatures((current) => ({
          ...current,
          [profile]: result.verified ? `vendor-signed (${result.key_id})` : "NOT verified",
        })),
      )
      .catch((reason: unknown) => setSignatures((current) => ({ ...current, [profile]: String(reason) })));
  };
  return (
    <section>
      <SectionHead title="Installer Catalog" detail={error ?? "Versioned TOML profiles embedded in the workspace."} />
      <table>
        <thead>
          <tr><th>Profile</th><th>Vendor</th><th>Strategies</th><th>Signature chain</th></tr>
        </thead>
        <tbody>
          {(data ?? previewProfiles).map((profile) => (
            <tr key={profile.id}>
              <td>{profile.id}</td>
              <td>{profile.vendor}</td>
              <td>{profile.supported_staging_strategies.join(", ")}</td>
              <td>
                {profile.signing_key ? (
                  signatures[profile.id] ?? (
                    <button className="copy" disabled={!isLive()} onClick={() => verify(profile.id)}>Verify</button>
                  )
                ) : "no pinned vendor key (fails closed)"}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function Planner() {
  const [profile, setProfile] = useState("ubuntu-24.04-desktop");
  const { data, error } = useBridge<Record<string, unknown>>(() => bridge.planPreview("boot-once", profile), [profile]);
  const steps = (data?.steps as Array<{ id: string; title: string; risk: string }> | undefined) ?? [];
  return (
    <section>
      <SectionHead title="Plan Preview" detail={error ?? "Boot-once plan for this host; execution stays blocked."} risk={data ? String(data.risk) : undefined} />
      <ProfilePicker value={profile} onChange={setProfile} />
      <ul className="steps">
        {steps.map((step) => (
          <li key={step.id}><strong>{step.risk}</strong><span>{step.title}</span></li>
        ))}
      </ul>
    </section>
  );
}

function Reports() {
  const { data, error, refresh } = useBridge<SavedReport[]>(bridge.reports);
  const [message, setMessage] = useState<string | null>(null);
  const act = (planId: string, action: "resume" | "rollback") =>
    bridge
      .recover(planId, action)
      .then(() => {
        setMessage(`${action} completed for ${planId}`);
        refresh();
      })
      .catch((reason: unknown) => setMessage(String(reason)));
  const simulate = () =>
    bridge
      .simulate("boot-once", "ubuntu-24.04-desktop", 2)
      .then(() => {
        setMessage("Saved an interrupted boot-once simulation.");
        refresh();
      })
      .catch((reason: unknown) => setMessage(String(reason)));
  return (
    <section>
      <SectionHead title="Execution Reports" detail={message ?? error ?? "Saved simulations with their recovery recommendation."} />
      <button className="copy" disabled={!isLive()} onClick={simulate}>Simulate an interrupted boot-once</button>
      <table>
        <thead><tr><th>Plan</th><th>Status</th><th>Recommended</th><th>Action</th></tr></thead>
        <tbody>
          {(data ?? []).map((report) => (
            <tr key={report.summary.plan_id}>
              <td>{report.summary.plan_id}</td>
              <td>{report.summary.status}</td>
              <td title={report.recovery?.reasons.join("\n")}>{report.recovery?.recommended ?? "unreadable"}</td>
              <td>
                {report.recovery?.recommended === "resume" && (
                  <button className="copy" onClick={() => act(report.summary.plan_id, "resume")}>Resume</button>
                )}
                {(report.recovery?.recommended === "resume" || report.recovery?.recommended === "rollback") && (
                  <button className="copy" onClick={() => act(report.summary.plan_id, "rollback")}>Roll back</button>
                )}
              </td>
            </tr>
          ))}
        </tbody>
      </table>
    </section>
  );
}

function Diagnostics() {
  return (
    <section>
      <SectionHead title="Diagnostics" detail="Project-local bundle export with secret-like line redaction, and the CLI equivalents of every view." />
      <div className="commands">
        {commands.map((command) => (
          <div className="command-row" key={command}>
            <pre>{command}</pre>
            <button className="copy" onClick={() => navigator.clipboard?.writeText(command)}>Copy</button>
          </div>
        ))}
      </div>
    </section>
  );
}

const previewProfiles: Profile[] = [
  { id: "ubuntu-24.04-desktop", name: "Ubuntu Desktop", vendor: "Canonical", version: "24.04.4 LTS", supported_staging_strategies: ["iso_loopback", "kernel_initrd"], signing_key: "ubuntu-cdimage-2012" },
  { id: "ubuntu-24.04-server", name: "Ubuntu Server", vendor: "Canonical", version: "24.04.4 LTS", supported_staging_strategies: ["iso_loopback", "kernel_initrd"], signing_key: "ubuntu-cdimage-2012" },
  { id: "fedora-44-workstation", name: "Fedora Workstation", vendor: "Fedora Project", version: "44", supported_staging_strategies: ["iso_loopback"], signing_key: "fedora-44" },
  { id: "windows-11", name: "Windows 11", vendor: "Microsoft", version: "11", supported_staging_strategies: ["windows_mounted_iso_setup"], signing_key: null },
];

function ProfilePicker({ value, onChange, extra = [] }: { value: string; onChange: (value: string) => void; extra?: string[] }) {
  return (
    <p>
      <label>
        Profile{" "}
        <select value={value} onChange={(event) => onChange(event.target.value)}>
          {previewProfiles.map((profile) => (
            <option key={profile.id} value={profile.id}>{profile.id}</option>
          ))}
          {extra.map((id) => (
            <option key={id} value={id}>{id === CUSTOM_PROFILE ? "custom (your own Linux live ISO)" : id}</option>
          ))}
        </select>
      </label>
    </p>
  );
}

function keyResults(report: Record<string, unknown>): string {
  const keys = [
    "backend",
    "one_shot_selection_proven",
    "power_loss_matrix_complete",
    "full_installer_payload_in_ram",
    "secure_boot_enabled",
    "signed_chain_accepted",
    "unsigned_loader_rejected",
    "host_boot_changes",
  ];
  return keys
    .filter((key) => key in report)
    .map((key) => `${key}=${String(report[key])}`)
    .join("  ");
}

function tri(value: boolean | null): string {
  return value === null ? "unknown" : value ? "yes" : "no";
}

function Row({ name, value }: { name: string; value: string }) {
  return (
    <tr>
      <th>{name}</th>
      <td>{value}</td>
    </tr>
  );
}

function SectionHead({ title, detail, risk }: { title: string; detail: string; risk?: string }) {
  return (
    <div className="section-head">
      <div>
        <h2>{title}</h2>
        <p className="muted">{detail}</p>
      </div>
      {risk ? <span className="risk high">{risk}</span> : null}
    </div>
  );
}

function Panel({ title, text }: { title: string; text: string }) {
  return (
    <div className="panel">
      <h3>{title}</h3>
      <p>{text}</p>
    </div>
  );
}

function label(value: string) {
  return value.charAt(0).toUpperCase() + value.slice(1);
}

createRoot(document.getElementById("root")!).render(<App />);
