/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */

const sidebars = {
  documentationSidebar: [
    {
      type: "category",
      label: "Getting Started",
      collapsed: false,
      items: ["installation", "quick_start", "cli", "sandbox", "transfer"],
    },

    {
      type: "category",
      label: "Architecture",
      collapsed: false,
      items: [
        "architecture/overview",
        "architecture/bridge",
        "architecture/accounts",
      ],
    },

    {
      type: "category",
      label: "Smart functions",
      collapsed: false,
      items: [
        "functions/overview",
        "functions/building",
        "functions/deploying",
        "functions/requests",
        "functions/data_storage",
        "functions/calling",
        "functions/tokens",
        "functions/errors",
      ],
    },

    {
      type: "category",
      label: "API Reference",
      collapsed: true,
      items: [
        "api/index",
        "api/console",
        "api/kv",
        "api/smart_function",
        "api/ledger",
        "api/headers",
        "api/request",
        "api/response",
        "api/url",
        "api/url_search_params",
        "api/url_pattern",
        "api/text_encoder",
        "api/text_decoder",
      ],
    },
  ],
};

module.exports = sidebars;
