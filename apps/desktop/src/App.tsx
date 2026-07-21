import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

const views = ["overview", "delivery", "vm evidence", "catalog", "planner", "reports", "diagnostics"] as const;

const commands = [
  "cargo run -p nousb -- tui",
  "cargo run -p nousb -- vm doctor",
  "cargo run -p nousb -- vm rehearse windows-bcd-bootsequence ubuntu-24.04-desktop",
  "cargo run -p nousb -- vm qemu plan-smoke",
  "cargo run -p nousb -- vm qemu plan-ubuntu-netboot",
  "cargo run -p nousb -- vm qemu plan-ubuntu-full-ram",
  "cargo run -p nousb -- vm qemu plan-ubuntu-uefi-full-ram",
  "cargo run -p nousb -- vm qemu rehearse-ubuntu-uefi-full-ram \"RUN DISPOSABLE UBUNTU UEFI FULL ISO RAM REHEARSAL\"",
  "cargo run -p nousb -- --json image manifest ubuntu-24.04-desktop",
  "cargo run -p nousb -- system inspect-fake windows-bitlocker",
  "cargo run -p nousb -- plan-fake linux-luks-low-space boot-once ubuntu-24.04-desktop",
  "cargo run -p nousb -- --interrupt-after 2 simulate boot-once ubuntu-24.04-desktop",
];

function App() {
  const [view, setView] = useState<(typeof views)[number]>("overview");

  return (
    <main>
      <header>
        <div>
          <p className="eyebrow">NoUSB OS Switcher</p>
          <h1>Safe installer staging, preflight planning, and recovery rehearsal</h1>
        </div>
        <div className="statusbar" aria-label="Safety status">
          <span className="pill">Default boot changes disabled</span>
          <span className="pill">Profiles validated</span>
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
          {view === "overview" && (
            <section>
              <SectionHead title="System Summary" detail="Static preview wired to the current safe CLI model." />
              <div className="grid">
                <Panel title="Guided TUI" text="Run nousb tui for profile selection, feasibility checks, verified acquisition, staging, and plan previews." />
                <Panel title="Fake Scenarios" text="Windows BitLocker and Linux LUKS low-space. macOS is outside the current objective." />
                <Panel title="Local State" text="Verified images, staging bundles, reports, diagnostics, QEMU runtime, and disposable VM evidence remain project-local." />
              </div>
            </section>
          )}

          {view === "catalog" && (
            <section>
              <SectionHead title="Installer Catalog" detail="Versioned TOML profiles embedded in the workspace." />
              <table>
                <thead>
                  <tr>
                    <th>Profile</th>
                    <th>Vendor</th>
                    <th>Strategies</th>
                    <th>Artifact</th>
                  </tr>
                </thead>
                <tbody>
                  <tr><td>ubuntu-24.04-desktop</td><td>Canonical</td><td>ISO loopback, kernel/initrd</td><td>Pinned 24.04.4</td></tr>
                  <tr><td>ubuntu-24.04-server</td><td>Canonical</td><td>UEFI FAT, network-to-RAM</td><td>Pinned 24.04.4</td></tr>
                  <tr><td>fedora-44-workstation</td><td>Fedora Project</td><td>ISO loopback</td><td>Pinned 44</td></tr>
                  <tr><td>windows-11</td><td>Microsoft</td><td>Mounted ISO setup</td><td>Vendor-mediated</td></tr>
                </tbody>
              </table>
            </section>
          )}

          {view === "delivery" && (
            <section>
              <SectionHead title="No-USB Delivery" detail="Feasibility before planning or execution." />
              <div className="grid">
                <Panel title="Recommended design" text="Acquire the pinned official ISO, extract only declared Ubuntu boot assets, then copy live media to RAM after boot begins." />
                <Panel title="Firmware alternative" text="UEFI HTTP Boot avoids disk staging only when the machine firmware and preboot network device support it." />
                <Panel title="Recovery rehearsal" text="Four one-shot backend models pass every simulated interruption; real QEMU now proves a pinned full Ubuntu Server ISO can continue from RAM after both EFI and HTTP sources are removed." />
                <Panel title="Current boundary" text="Secure Boot, guest-side HTTPS, a real current-OS fallback boot, power-loss recovery, and host boot adapters remain unproven. Host execution stays disabled." />
              </div>
            </section>
          )}

          {view === "vm evidence" && (
            <section>
              <SectionHead title="Disposable VM Evidence" detail="Real QEMU runs, kept separate from host boot execution." />
              <div className="grid">
                <Panel title="UEFI + official assets: proven" text="OVMF selected the run-local FAT source; pinned BOOTX64.EFI, GRUB, and MOK manager hashes matched the official Ubuntu 24.04.4 Server ISO." />
                <Panel title="Full installer RAM: proven" text="The complete 3,405,469,696-byte ISO was saved in initramfs, Subiquity mounted, and the installer tty appeared after the HTTP source stopped." />
                <Panel title="Boot-source removal: proven" text="QMP deleted the boot USB and vvfat node; the final block list contained only immutable OVMF code and a disposable variables copy." />
                <Panel title="Remaining gates" text="Secure Boot is off, the guest handoff is isolated HTTP rather than HTTPS, and a real one-shot fallback/current-OS reboot is still unproven." />
              </div>
            </section>
          )}

          {view === "planner" && (
            <section>
              <SectionHead title="Plan Preview" detail="Fake Linux low-space boot-once plan for Ubuntu." risk="High" />
              <ul className="steps">
                <li><strong>boot</strong><span>Prefer one-time boot, keep the current OS default, and disallow permanent boot-order changes.</span></li>
                <li><strong>info</strong><span>Inspect current firmware, encryption, disk, and boot metadata.</span></li>
                <li><strong>low</strong><span>Create a project-local recovery pack.</span></li>
                <li><strong>medium</strong><span>Verify installer checksum before any execution path.</span></li>
                <li><strong>medium</strong><span>Preview staging without boot changes.</span></li>
                <li><strong>high</strong><span>Resolve low disk space before proceeding.</span></li>
              </ul>
            </section>
          )}

          {view === "reports" && (
            <section>
              <SectionHead title="Execution Reports" detail="Simulation and disposable real-VM evidence survive as project-local JSON." />
              <div className="grid">
                <Panel title="Interrupted" text="Recovery required, pending steps preserved." />
                <Panel title="Rolled Back" text="Rollback-capable completed steps marked." />
                <Panel title="Failed" text="Named step failure can be rehearsed with --fail-step." />
              </div>
            </section>
          )}

          {view === "diagnostics" && (
            <section>
              <SectionHead title="Diagnostics" detail="Project-local bundle export with secret-like line redaction." />
              <div className="commands">
                {commands.map((command) => (
                  <div className="command-row" key={command}>
                    <pre>{command}</pre>
                    <button className="copy" onClick={() => navigator.clipboard?.writeText(command)}>Copy</button>
                  </div>
                ))}
              </div>
            </section>
          )}
        </div>
      </div>
    </main>
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
