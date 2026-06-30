import React, { useState } from "react";
import { createRoot } from "react-dom/client";
import "./style.css";

const views = ["overview", "catalog", "planner", "reports", "diagnostics"] as const;

const commands = [
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
                <Panel title="Host Adapter" text="host-safe inspection only. Real boot adapters disabled by default." />
                <Panel title="Fake Scenarios" text="Windows BitLocker, Linux LUKS low-space, macOS Apple Silicon." />
                <Panel title="Local State" text="Reports in .nousb/reports; bundles in .nousb/diagnostics." />
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
                    <th>Free Space</th>
                  </tr>
                </thead>
                <tbody>
                  <tr><td>ubuntu-24.04-desktop</td><td>Canonical</td><td>ISO loopback, kernel/initrd</td><td>8 GB</td></tr>
                  <tr><td>fedora-42-workstation</td><td>Fedora Project</td><td>ISO loopback</td><td>8 GB</td></tr>
                  <tr><td>windows-11</td><td>Microsoft</td><td>Mounted ISO setup</td><td>16 GB</td></tr>
                </tbody>
              </table>
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
              <SectionHead title="Execution Reports" detail="Simulation reports survive across sessions as JSON." />
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
