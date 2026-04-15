import { themes as prismThemes } from "prism-react-renderer";
import type { Config } from "@docusaurus/types";
import type * as Preset from "@docusaurus/preset-classic";

const vercelUrl = process.env.VERCEL_URL
  ? `https://${process.env.VERCEL_URL}`
  : undefined;
const siteUrl =
  process.env.DOCS_SITE_URL ?? vercelUrl ?? "https://prismo.alextac.com";
const baseUrl = process.env.DOCS_BASE_URL ?? "/";
const repoUrl =
  process.env.DOCS_REPO_URL ?? "https://github.com/alextac98/prismo";

const config: Config = {
  title: "Prismo",
  tagline: "An adaptable terminal telemetry viewer.",
  future: {
    v4: {
      removeLegacyPostBuildHeadAttribute: false,
      useCssCascadeLayers: false,
      siteStorageNamespacing: false,
      fasterByDefault: false,
      mdx1CompatDisabledByDefault: false,
    },
  },
  url: siteUrl,
  baseUrl,
  onBrokenLinks: "throw",
  markdown: {
    hooks: {
      onBrokenMarkdownLinks: "throw",
    },
  },
  i18n: {
    defaultLocale: "en",
    locales: ["en"],
  },
  presets: [
    [
      "classic",
      {
        docs: {
          path: "../docs",
          routeBasePath: "docs",
          sidebarPath: "./sidebars.ts",
        },
        blog: false,
        theme: {
          customCss: "./src/css/custom.css",
        },
      } satisfies Preset.Options,
    ],
  ],
  themeConfig: {
    colorMode: {
      defaultMode: "light",
      disableSwitch: false,
      respectPrefersColorScheme: true,
    },
    navbar: {
      title: "Prismo",
      items: [
        {
          type: "docSidebar",
          sidebarId: "tutorialSidebar",
          position: "left",
          label: "Docs",
        },
        {
          href: repoUrl,
          position: "right",
          className: "header-github-link",
          "aria-label": "GitHub repository",
        },
      ],
    },
    footer: {
      style: "dark",
      links: [],
      copyright: `<div class="footer-row"><span>© ${new Date().getFullYear()} Alex Tacescu</span><a href="${repoUrl}" target="_blank" rel="noopener noreferrer">GitHub</a></div>`,
    },
    prism: {
      theme: prismThemes.github,
      darkTheme: prismThemes.dracula,
    },
  } satisfies Preset.ThemeConfig,
};

export default config;
