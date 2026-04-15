import type { ReactNode } from "react";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import useBaseUrl from "@docusaurus/useBaseUrl";
import Layout from "@theme/Layout";

import styles from "./index.module.css";

const features = [
  {
    title: "Terminal First",
    body: "See your live telemetry in a friendly TUI built for quick debugging and fast iteration.",
  },
  {
    title: "Plugin Extensible",
    body: "Connect Prismo to your own system with plugins, whether you are working on embedded targets or Linux hosts.",
  },
  {
    title: "Simple and Intuitive",
    body: "Stay close to the system you are debugging with a compact UI, live filters, and copy-friendly output.",
  },
];

export default function Home(): ReactNode {
  const { siteConfig } = useDocusaurusContext();
  const screenshotUrl = useBaseUrl("/img/prismo-app-screenshot.png");

  return (
    <Layout
      title={siteConfig.title}
      description="Prismo is a terminal-first telemetry viewer for debugging systems with live data."
    >
      <main className={styles.page}>
        {/* ── Hero ── */}
        <section className={styles.hero}>
          <h1 className={styles.heroTitle}>Prismo</h1>
          <p className={styles.heroTagline}>
            <span className={styles.promptChar}>$</span> Live telemetry in the
            terminal.<span className={styles.blink}>_</span>
          </p>
          <p className={styles.heroSubtext}>
            Prismo is a fast telemetry viewer for debugging systems. Connect it
            to your robot, spacecraft, or other system by building your own
            plugin and inspect live data without leaving the shell.
          </p>
          <div className={styles.heroScreenshot}>
            <div className={styles.termFrame}>
              <div className={styles.termBar}>
                <span className={styles.termDot} data-color="red" />
                <span className={styles.termDot} data-color="yellow" />
                <span className={styles.termDot} data-color="green" />
                <span className={styles.termBarTitle}>prismo</span>
              </div>
              <img
                alt="Prismo terminal telemetry viewer"
                className={styles.screenshot}
                src={screenshotUrl}
              />
            </div>
          </div>
        </section>

        {/* ── Features: ASCII-bordered cards ── */}
        <section className={styles.featuresSection}>
          <div className={styles.sectionLine}>
            <span className={styles.lineLabel}>{"── features "}</span>
            <span className={styles.lineFill} />
          </div>
          <div className={styles.featureGrid}>
            {features.map((f) => (
              <div key={f.title} className={styles.featureCard}>
                <h3 className={styles.featureTitle}>{f.title}</h3>
                <p className={styles.featureBody}>{f.body}</p>
              </div>
            ))}
          </div>
        </section>
      </main>
    </Layout>
  );
}
