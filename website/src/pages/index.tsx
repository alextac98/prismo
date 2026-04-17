import { useEffect, useState } from "react";
import type { CSSProperties, ReactNode } from "react";
import useDocusaurusContext from "@docusaurus/useDocusaurusContext";
import useBaseUrl from "@docusaurus/useBaseUrl";
import Layout from "@theme/Layout";

import homepageDemo from "../homepageDemoData";
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

function segmentStyle(
  style: (typeof homepageDemo.styles)[number],
): CSSProperties {
  return {
    ...(style.fg ? { color: style.fg } : {}),
    ...(style.bg ? { backgroundColor: style.bg } : {}),
    ...(style.bold ? { fontWeight: 700 } : {}),
    ...(style.dim ? { opacity: 0.72 } : {}),
    ...(style.italic ? { fontStyle: "italic" } : {}),
    ...(style.underlined ? { textDecoration: "underline" } : {}),
  };
}

function HomepageTerminalDemo({
  screenshotUrl,
}: {
  screenshotUrl: string;
}): ReactNode {
  const [frameIndex, setFrameIndex] = useState(0);
  const [paused, setPaused] = useState(false);
  const [reducedMotion, setReducedMotion] = useState(false);
  const frameCount = homepageDemo.frames.length;

  useEffect(() => {
    if (typeof window === "undefined") {
      return undefined;
    }

    const media = window.matchMedia("(prefers-reduced-motion: reduce)");
    const update = () => setReducedMotion(media.matches);
    update();
    media.addEventListener("change", update);
    return () => media.removeEventListener("change", update);
  }, []);

  useEffect(() => {
    if (reducedMotion || paused || frameCount === 0) {
      return undefined;
    }

    const currentFrame = homepageDemo.frames[frameIndex];
    const timeoutId = window.setTimeout(() => {
      setFrameIndex((frameIndex + 1) % frameCount);
    }, currentFrame.durationMs);

    return () => window.clearTimeout(timeoutId);
  }, [frameCount, frameIndex, paused, reducedMotion]);

  if (reducedMotion) {
    return (
      <img
        alt="Prismo terminal telemetry viewer"
        className={styles.screenshot}
        src={screenshotUrl}
      />
    );
  }

  const frame = homepageDemo.frames[frameIndex] ?? homepageDemo.frames[0];

  return (
    <div
      className={styles.demoViewport}
      onFocus={() => setPaused(true)}
      onBlur={() => setPaused(false)}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
    >
      <div className={styles.demoMeta}>
        <span className={styles.demoLabel}>Build-time live demo</span>
        <button
          className={styles.demoButton}
          onClick={() => setFrameIndex(0)}
          type="button"
        >
          Restart
        </button>
      </div>
      <div
        aria-label="Animated terminal demo of the Prismo TUI"
        className={styles.demoScreen}
        role="img"
      >
        {frame.lines.map((line, lineIndex) => (
          <div key={lineIndex} className={styles.demoLine}>
            {line.map((segment, segmentIndex) => (
              <span
                key={`${lineIndex}-${segmentIndex}`}
                style={segmentStyle(homepageDemo.styles[segment.style])}
              >
                {segment.text}
              </span>
            ))}
          </div>
        ))}
      </div>
    </div>
  );
}

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
              <HomepageTerminalDemo screenshotUrl={screenshotUrl} />
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
