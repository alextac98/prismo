import type {ReactNode} from 'react';
import clsx from 'clsx';
import Link from '@docusaurus/Link';
import useDocusaurusContext from '@docusaurus/useDocusaurusContext';
import useBaseUrl from '@docusaurus/useBaseUrl';
import Layout from '@theme/Layout';
import Heading from '@theme/Heading';

import styles from './index.module.css';

function FeatureCard(props: {title: string; body: string}) {
  return (
    <article className={styles.featureCard}>
      <h3>{props.title}</h3>
      <p>{props.body}</p>
    </article>
  );
}

export default function Home(): ReactNode {
  const {siteConfig} = useDocusaurusContext();
  const screenshotUrl = useBaseUrl('/img/prismo-app-screenshot.png');

  return (
    <Layout
      title={siteConfig.title}
      description="Prismo is a protocol-first terminal telemetry viewer for embedded and target-side debugging.">
      <main className={styles.page}>
        <section className={styles.hero}>
          <div className={styles.heroText}>
            <div className={styles.kicker}>Protocol-first telemetry tooling</div>
            <Heading as="h1" className={styles.title}>
              Inspect live telemetry without leaving the terminal.
            </Heading>
            <p className={styles.subtitle}>{siteConfig.tagline}</p>
            <p className={styles.copy}>
              Prismo combines a Rust TUI, a subprocess plugin model, and a
              Bazel-first workspace so you can explore streams, chart numeric
              history, and grow toward multi-language plugins.
            </p>
            <div className={styles.actions}>
              <Link className={clsx('button button--primary button--lg', styles.primaryAction)} to="/docs">
                Read the docs
              </Link>
              <Link className={clsx('button button--secondary button--lg', styles.secondaryAction)} to="/docs/getting-started">
                Run the prototype
              </Link>
            </div>
          </div>
          <div className={styles.heroVisual}>
            <div className={styles.screenshotFrame}>
              <img
                alt="Prismo terminal telemetry viewer screenshot"
                className={styles.screenshot}
                src={screenshotUrl}
              />
            </div>
          </div>
        </section>

        <section className={styles.featureGrid}>
          <FeatureCard
            title="Terminal-native workflow"
            body="Keep focus in the shell with fast keyboard navigation, copy support, filters, and a compact status bar."
          />
          <FeatureCard
            title="Shared plugin protocol"
            body="Rust and C++ examples already run through the same protobuf-over-stdio boundary, with room for more SDKs."
          />
          <FeatureCard
            title="Bazel-first repository"
            body="Hermetic toolchains and workspace-level dependency control stay central, while Cargo metadata remains available for Rust development."
          />
        </section>

        <section className={styles.quickstart}>
          <div>
            <div className={styles.sectionLabel}>Quickstart</div>
            <Heading as="h2" className={styles.sectionTitle}>
              Current local run path
            </Heading>
            <p className={styles.sectionCopy}>
              The prototype is easiest to launch through Cargo today, while
              Bazel remains the main build and test surface.
            </p>
          </div>
          <pre className={styles.commandBlock}>
            <code>{`cargo run -q -- --plugins ./plugins/example-rust
bazel build //apps/prismo
bazel test //apps/prismo:cpp_smoke_test`}</code>
          </pre>
        </section>

        <section className={styles.architecture}>
          <div>
            <div className={styles.sectionLabel}>Architecture</div>
            <Heading as="h2" className={styles.sectionTitle}>
              A small but deliberate runtime split
            </Heading>
            <p className={styles.sectionCopy}>
              The current workspace separates telemetry ingestion, protocol
              contracts, subprocess hosting, TUI rendering, and SDK surfaces so
              the plugin boundary stays explicit as the prototype grows.
            </p>
          </div>
          <div className={styles.flowCard}>
            <div className={styles.flowLabel}>Data flow</div>
            <code className={styles.flowCode}>
              plugin subprocess -&gt; protobuf frames -&gt; plugin-host -&gt; app -&gt;
              store snapshot -&gt; TUI
            </code>
            <Link className={styles.inlineLink} to="/docs/architecture">
              Read the architecture guide
            </Link>
          </div>
        </section>
      </main>
    </Layout>
  );
}
