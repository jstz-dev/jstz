---
title: 👨‍⚖️ jstz
---

<script setup>
import VPButton from "vitepress/dist/client/theme-default/components/VPButton.vue";
</script>

# 👨‍⚖️ Jstz

Jstz (pronounced "justice") is a JavaScript server runtime for Tezos [Smart Rollups](https://docs.tezos.com/architecture/smart-rollups) with a great developer experience.
With Jstz, you can deploy JavaScript applications known as _smart functions_ that can act as the backend for web applications, including handling logic, storing data, and accepting and distributing payments.

In particular, Jstz is:

- 🚀 **Fast**: Jstz is built on [Boa](https://boajs.dev/), a blazingly fast JavaScript engine written in Rust.
- 📚 **Easy to learn**: Jstz is built with the developer in mind.
- ⚡️ **Fully local**: You can test and develop smart functions locally with a sandbox.

The Jstz command-line toolkit, sandbox, SDK, and other tools in this repository are free and open source software under the [MIT license](https://github.com/jstz-dev/jstz/blob/main/LICENSE).

<VPButton href="/quick_start" size="big" theme="alt" text="Get Started!" style="border-radius:4px;text-decoration:none" />

Jstz smart functions look like ordinary JavaScript functions, but they have some differences and limitations; see [Smart functions](/functions/overview).
