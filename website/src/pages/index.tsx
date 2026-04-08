import type { ReactNode } from "react";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import useBaseUrl from "@docusaurus/useBaseUrl";
import Layout from "@theme/Layout";

import styles from "./index.module.css";

const features = [
  {
    title: "Terminal-native",
    body: "Stay in the shell. Fast keyboard\nnavigation, copy support, live\nfilters, and a compact status bar.",
  },
  {
    title: "Plugin protocol",
    body: "Rust and C++ plugins share the\nsame protobuf-over-stdio boundary.\nBring your own language.",
  },
  {
    title: "Bazel-first",
    body: "Hermetic toolchains and workspace-\nlevel dependency control. Cargo\nmetadata available for IDE support.",
  },
];

export default function Home(): ReactNode {
  const { siteConfig } = useDocusaurusContext();
  const screenshotUrl = useBaseUrl("/img/prismo-app-screenshot.png");

  return (
    <Layout
      title={siteConfig.title}
      description="Prismo is a protocol-first terminal telemetry viewer for embedded and target-side debugging."
    >
      <main className={styles.page}>
        {/* ── Hero ── */}
        <section className={styles.hero}>
          <h1 className={styles.heroTitle}>Prismo</h1>
          <p className={styles.heroTagline}>
            <span className={styles.promptChar}>$</span> Live telemetry, right
            in the terminal.<span className={styles.blink}>_</span>
          </p>
          <p className={styles.heroSubtext}>
            {siteConfig.tagline} View your data directly from the source, and
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
