import { defineConfig } from "vitepress";

// https://vitepress.dev/reference/site-config
export default defineConfig({
  titleTemplate: false,
  title: "👨‍⚖️ jstz",
  description: "A JavaScript server runtime that powers Tezos 2.0",
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
          { text: "First Steps", link: "/first_steps" },
          { text: "CLI", link: "/cli" },
        ],
      },

      {
        text: "API Reference",
        items: [
          { text: "Overview", link: "/api/" },
          { text: "KV", link: "/api/kv" },
          { text: "Ledger", link: "/api/ledger" },
          { text: "Headers", link: "/api/headers" },
          { text: "Response", link: "/api/response" },
        ],
      },
    ],

    socialLinks: [
      { icon: "github", link: "https://github.com/trilitech/jstz/" },
    ],
  },
});
