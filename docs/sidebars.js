/** @type {import('@docusaurus/plugin-content-docs').SidebarsConfig} */

const sidebars = {
  documentationSidebar: [
    {
      type: "category",
      collapsed: false,
      label: "Getting Started",
      items: ["installation", "quick_start", "cli", "sandbox", "examples"],
    },

    {
      type: "category",
      collapsed: false,
      label: "Architecture",
      items: [
        "architecture/overview",
        "architecture/bridge",
        "architecture/accounts",
        "architecture/networks",
        "architecture/oracle",
      ],
    },

    {
      type: "category",
      collapsed: false,
      label: "Smart functions",
      items: [
        "functions/overview",
        "functions/building",
        "functions/deploying",
        "functions/requests",
        "functions/data_storage",
        "functions/calling",
        "functions/apis",
        "functions/tokens",
        "functions/errors",
      ],
    },

    {
      type: "category",
      collapsed: true,
      label: "API Reference",
      items: [
        "api/index",
        "api/console",
        "api/kv",
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
