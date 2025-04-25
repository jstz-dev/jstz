import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  titleTemplate: false,
  title: "👨‍⚖️ jstz",
  description: "A JavaScript runtime powered by Tezos smart optimistic rollups",
  lang: "en-US",
  head: [
    [
      "link",
      {
        rel: "icon",
        href: "data:image/svg+xml,<svg xmlns=%22http://www.w3.org/2000/svg%22 viewBox=%220 0 100 100%22><text y=%22.9em%22 font-size=%2290%22>👨‍⚖️</text></svg>",
      },
    ],
  ],
  base: "/jstz/",
  themeConfig: {
    search: {
      provider: "local",
    },

    sidebar: [
      {
        text: "Getting Started",
        items: [
          { text: "Installation", link: "/installation" },
          { text: "Quick Start", link: "/quick_start" },
          { text: "CLI", link: "/cli" },
          { text: "Sandbox", link: "/sandbox" },
        ],
      },

      {
        text: "Architecture",
        items: [
          { text: "Overview", link: "/architecture/overview" },
          { text: "Asset bridge", link: "/architecture/bridge" },
          { text: "Accounts", link: "/architecture/accounts" },
        ],
      },

      {
        text: "Smart functions",
        items: [
          { text: "Overview", link: "/functions/overview" },
          { text: "Building", link: "/functions/building" },
          { text: "Deploying", link: "/functions/deploying" },
          { text: "Handling requests", link: "/functions/requests" },
          { text: "Storing data", link: "/functions/data_storage" },
          { text: "Calling other smart functions", link: "/functions/calling" },
          { text: "Managing tokens", link: "/functions/tokens" },
          { text: "Handling errors", link: "/functions/errors" },
        ],
      },

      {
        text: "API Reference",
        items: [
          { text: "Overview", link: "/api/" },
          { text: "Console", link: "/api/console" },
          { text: "KV", link: "/api/kv" },
          { text: "SmartFunction", link: "/api/smart_function" },
          { text: "Ledger", link: "/api/ledger" },
          { text: "Headers", link: "/api/headers" },
          { text: "Request", link: "/api/request" },
          { text: "Response", link: "/api/response" },
          { text: "URL", link: "/api/url" },
          { text: "URLSearchParams", link: "/api/url_search_params" },
          { text: "URLPattern", link: "/api/url_pattern" },
          { text: "TextEncoder", link: "/api/text_encoder" },
          { text: "TextDecoder", link: "/api/text_decoder" },
        ],
      },
    ],

    socialLinks: [
      { icon: "github", link: "https://github.com/jstz-dev/jstz/" },
    ],
  },
});
