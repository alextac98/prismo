import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

const features = [
  {
    icon: '>_',
    title: 'Terminal-native',
    body: 'Stay in the shell. Fast keyboard navigation, copy support, live filters, and a compact status bar.',
  },
  {
    icon: '{}',
    title: 'Plugin protocol',
    body: 'Rust and C++ plugins share the same protobuf-over-stdio boundary. Bring your own language.',
  },
  {
    icon: '//',
    title: 'Bazel-first',
    body: 'Hermetic toolchains and workspace-level dependency control. Cargo metadata available for IDE support.',
  },
];

const steps = [
  {label: 'Run with Cargo', command: 'cargo run -q -- --plugins ./plugins/example-rust'},
  {label: 'Build with Bazel', command: 'bazel build //apps/prismo'},
  {label: 'Test', command: 'bazel test //apps/prismo:cpp_smoke_test'},
];

const flowSteps = [
  'plugin subprocess',
  'protobuf frames',
  'plugin-host',
  'app',
  'store snapshot',
  'TUI',
];

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  const screenshotUrl = useBaseUrl('/img/prismo-app-screenshot.png');

  return (
    <Layout
      title={siteConfig.title}
      description="Prismo is a protocol-first terminal telemetry viewer for embedded and target-side debugging.">
      <main className={styles.page}>
        {/* ── Hero ── */}
        <section className={styles.hero}>
          <span className={styles.badge}>Open-source telemetry tooling</span>
          <Heading as="h1" className={styles.title}>
            Live telemetry,{'\n'}
            <span className={styles.titleAccent}>right in the terminal.</span>
          </Heading>
          <p className={styles.subtitle}>
            {siteConfig.tagline} Explore streams, chart numeric history, and
            extend with multi-language plugins.
          </p>
          <div className={styles.actions}>
            <Link
              className={clsx('button button--primary button--lg', styles.primaryBtn)}
              to="/docs">
              Get started
            </Link>
            <Link
              className={clsx('button button--outline button--lg', styles.secondaryBtn)}
              to="/docs/getting-started">
              View quickstart
            </Link>
          </div>
        </section>

        {/* ── Screenshot ── */}
        <section className={styles.screenshotSection}>
          <div className={styles.screenshotFrame}>
            <div className={styles.screenshotDots}>
              <span /><span /><span />
            </div>
            <img
              alt="Prismo terminal telemetry viewer"
              className={styles.screenshot}
              src={screenshotUrl}
            />
          </div>
        </section>

        {/* ── Features ── */}
        <section className={styles.features}>
          <div className={styles.sectionHeader}>
            <span className={styles.label}>Features</span>
            <Heading as="h2" className={styles.sectionTitle}>
              Built for the way you work
            </Heading>
          </div>
          <div className={styles.featureGrid}>
            {features.map((f) => (
              <article key={f.title} className={styles.featureCard}>
                <div className={styles.featureIcon}>{f.icon}</div>
                <h3 className={styles.featureTitle}>{f.title}</h3>
                <p className={styles.featureBody}>{f.body}</p>
              </article>
            ))}
          </div>
        </section>

        {/* ── Quickstart ── */}
        <section className={styles.quickstart}>
          <div className={styles.sectionHeader}>
            <span className={styles.label}>Quickstart</span>
            <Heading as="h2" className={styles.sectionTitle}>
              Up and running in seconds
            </Heading>
          </div>
          <div className={styles.terminal}>
            <div className={styles.terminalBar}>
              <span className={styles.terminalDots}>
                <span /><span /><span />
              </span>
              <span className={styles.terminalTitle}>terminal</span>
            </div>
            <div className={styles.terminalBody}>
              {steps.map((s) => (
                <div key={s.label} className={styles.terminalLine}>
                  <span className={styles.terminalComment}># {s.label}</span>
                  <code>
                    <span className={styles.terminalPrompt}>$ </span>
                    {s.command}
                  </code>
                </div>
              ))}
            </div>
          </div>
        </section>

        {/* ── Architecture ── */}
        <section className={styles.architecture}>
          <div className={styles.sectionHeader}>
            <span className={styles.label}>Architecture</span>
            <Heading as="h2" className={styles.sectionTitle}>
              A deliberate runtime split
            </Heading>
            <p className={styles.sectionCopy}>
              Telemetry ingestion, protocol contracts, subprocess hosting, TUI
              rendering, and SDK surfaces are cleanly separated so the plugin
              boundary stays explicit.
            </p>
          </div>
          <div className={styles.pipeline}>
            {flowSteps.map((step, i) => (
              <div key={step} className={styles.pipelineStep}>
                <div className={styles.pipelineNode}>{step}</div>
                {i < flowSteps.length - 1 && (
                  <div className={styles.pipelineArrow} aria-hidden="true" />
                )}
              </div>
            ))}
          </div>
          <div className={styles.archCta}>
            <Link className={styles.inlineLink} to="/docs/architecture">
              Read the architecture guide &rarr;
            </Link>
          </div>
        </section>
      </main>
    </Layout>
  );
}
