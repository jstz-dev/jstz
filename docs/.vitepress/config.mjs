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
  themeConfig: {
    search: {
      provider: "local",
    },

    sidebar: [
      {
        text: "Getting Started",
        items: [{ text: "👨‍⚖️ jstz", link: "/" }],
      },
    ],

    socialLinks: [
      { icon: "github", link: "https://github.com/trilitech/jstz/" },
    ],
  },
});
