import { defineConfig } from 'vitepress'

export default defineConfig({
  title: "Project",
  description: "Documentation",
  base: '/sharecli/',
  // Repo-root paths (crates/, scripts/, WORK_DAG.md, …) are intentional and
  // live outside the VitePress source tree — do not fail the docs build on them.
  ignoreDeadLinks: [
    /\/desktop\//,
    /\/windows\//,
    /\/crates\//,
    /\/scripts\//,
    /\/src\//,
    /\/audit\//,
    /\/systemd\//,
    /\/caddy\//,
    /\/patches\//,
    /\/README(#|\?|$)/,
    /\/CONTRIBUTING(#|\?|$)/,
    /\/CHANGELOG(#|\?|$)/,
    /WORK_DAG/,
    /TEST_COVERAGE_MATRIX/,
    /FUNCTIONAL_REQUIREMENTS/,
    /repro-check/,
    /\/brew-bottle/,
    /\/recovery\//,
    /\/Containerfile/,
    /\/assets\/brand\//,
    // Repo-root governance docs referenced from docs/ops — live outside the
    // VitePress source tree (no extension in link), so ignore dead-link check.
    /THREAT_MODEL(\.[a-z]+)?(#|\?|$)/,
    /\/SECURITY(\.md)?(#|\?|$)/,
    /\/BOUNDARY(\.md)?(#|\?|$)/,
  ],
  themeConfig: {
    nav: [
      { text: 'Home', link: '/' },
      { text: 'Journeys', link: '/journeys/' },
      { text: 'Stories', link: '/stories/' },
      { text: 'Traceability', link: '/traceability/' },
      { text: 'Compete', link: '/compete/' },
    ],
    sidebar: {
      '/journeys/': [{
        text: 'Journeys',
        items: [
          { text: 'Overview', link: '/journeys/' },
          { text: 'Quick Start', link: '/journeys/quick-start' },
        ]
      }],
      '/stories/': [{
        text: 'Stories',
        items: [
          { text: 'Overview', link: '/stories/' },
          { text: 'Hello World', link: '/stories/hello-world' },
        ]
      }],
      '/traceability/': [{
        text: 'Traceability',
        items: [
          { text: 'Overview', link: '/traceability/' },
        ]
      }],
    }
  }
})
