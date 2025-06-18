// @ts-nocheck
// Note: type annotations allow type checking and IDEs autocompletion

import { themes } from "prism-react-renderer";

// script-src causes development builds to fail
// But unsafe-eval should NOT be in production builds
// Also, put GTM first because sometimes the ';' in the escaped single quotes causes the browser to think it's the end
const scriptSrc =
  process.env.NODE_ENV === "development"
    ? `https://*.googletagmanager.com https://cdn.jsdelivr.net 'self' 'unsafe-inline' 'unsafe-eval'`
    : `https://*.googletagmanager.com https://cdn.jsdelivr.net 'self' 'unsafe-inline'`;

const contentSecurityPolicy = `
default-src 'none';
base-uri 'self';
manifest-src 'self';
script-src ${scriptSrc};
style-src https://cdn.jsdelivr.net https://fonts.googleapis.com 'self' 'unsafe-inline';
font-src https://cdn.jsdelivr.net https://fonts.gstatic.com 'self';
img-src 'self' https://*.googletagmanager.com https://*.google-analytics.com data: 'unsafe-eval';
media-src 'self';
form-action 'self';
connect-src 'self' https://*.algolia.net https://*.algolianet.com https://*.googletagmanager.com https://*.google-analytics.com https://*.analytics.google.com;`;

/** @type {import('@docusaurus/types').Config} */
module.exports = async function createConfigAsync() {
  return {
    title: "Jstz documentation",
    tagline: "A JavaScript runtime powered by Tezos smart optimistic rollups",
    favicon: "/img/favicon.svg",
    url: process.env.DOC_URL || "https://jstz-dev.github.io/",
    baseUrl: process.env.DOC_BASE_URL || "/jstz/",
    organizationName: "jstz-dev",
    projectName: "jstz",
    onBrokenLinks: "throw",
    onBrokenMarkdownLinks: "throw",
    onBrokenAnchors: "throw",
    i18n: {
      defaultLocale: "en",
      locales: ["en"],
    },

    headTags: [
      {
        tagName: "meta",
        attributes: {
          "http-equiv": "Content-Security-Policy",
          content: contentSecurityPolicy,
        },
      },
    ],

    presets: [
      [
        "classic",
        /** @type {import('@docusaurus/preset-classic').Options} */
        ({
          docs: {
            path: ".",
            exclude: ["node_modules/**/*"],
            include: [
              "index.md",
              "api/**/*.{md,mdx}",
              "architecture/**/*.{md,mdx}",
              "client/**/*.{md,mdx}",
              "functions/**/*.{md,mdx}",
              "cli.mdx",
              "installation.md",
              "quick_start.md",
              "sandbox.md",
              "transfer.md",
              "examples.md",
            ],
            sidebarPath: require.resolve("./sidebars.js"),
            routeBasePath: "/", // Serve the docs at the site's root
            showLastUpdateTime: false,
          },
          blog: false,
          theme: {
            customCss: require.resolve("./src/css/custom.css"),
          },
        }),
      ],
    ],

    plugins: [
      "plugin-image-zoom",
      [
        "@docusaurus/plugin-ideal-image",
        {
          quality: 70,
          max: 1030, // max resized image's size.
          min: 640, // min resized image's size. if original is lower, use that size.
          steps: 2, // the max number of images generated between min and max (inclusive)
          disableInDev: false,
        },
      ],
    ],

    themeConfig:
      /** @type {import('@docusaurus/preset-classic').ThemeConfig} */
      ({
        colorMode: {
          defaultMode: "light",
          disableSwitch: true,
          respectPrefersColorScheme: false,
        },
        // Replace with your project's social card
        // image: 'img/docusaurus-social-card.jpg',
        navbar: {
          style: "primary",
          title: "👨‍⚖️ Jstz docs",
          // logo: {
          //   alt: 'Developer docs for Jstz',
          //   src: 'img/logo-tezos.svg',
          // },
          items: [
            {
              href: "https://github.com/jstz-dev/jstz/",
              label: "GitHub",
              position: "right",
            },
          ],
        },
        prism: {
          theme: themes.github,
        },
        // https://github.com/flexanalytics/plugin-image-zoom
        // Enable click to zoom in to large images
        imageZoom: {
          // CSS selector to apply the plugin to, defaults to '.markdown img'
          selector: ".markdown img",
          // Optional medium-zoom options
          // see: https://www.npmjs.com/package/medium-zoom#options
          options: {
            margin: 24,
            scrollOffset: 0,
          },
        },
        algolia: {
          // The application ID provided by Algolia
          appId: process.env.NEXT_PUBLIC_DOCSEARCH_APP_ID || "XJJKSPLGTN",
          // Public API key: it is safe to commit it
          apiKey:
            process.env.NEXT_PUBLIC_DOCSEARCH_API_KEY ||
            "6173a0326b67c01cc1ee67a2bfea0adf",
          indexName:
            process.env.NEXT_PUBLIC_DOCSEARCH_INDEX_NAME || "jstz-devio",
          // Optional: see doc section below
          contextualSearch: true,
          // Optional: Specify domains where the navigation should occur through window.location instead on history.push. Useful when our Algolia config crawls multiple documentation sites and we want to navigate with window.location.href to them.
          // externalUrlRegex: 'external\\.com|domain\\.com',
          // Optional: Replace parts of the item URLs from Algolia. Useful when using the same search index for multiple deployments using a different baseUrl. You can use regexp or string in the `from` param. For example: localhost:3000 vs myCompany.com/docs
          // replaceSearchResultPathname: {
          //   from: '/docs/', // or as RegExp: /\/docs\//
          //   to: '/',
          // },
          // Optional: Algolia search parameters
          // searchParameters: {},
          // Optional: path for search page that enabled by default (`false` to disable it)
          searchPagePath: false,
          //... other Algolia params
        },
      }),
  };
};
