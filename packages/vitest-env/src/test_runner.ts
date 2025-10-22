import type {
  VitestRunner,
  VitestRunnerConfig,
  VitestRunnerImportSource,
  CancelReason,
  File,
  Test,
  Suite,
  TaskResultPack,
  TestAnnotation,
  TestContext,
  ImportDuration,
  TaskEventPack,
  TaskResult,
} from "@vitest/runner";

import { VitestExecutor } from "vitest/execute";

function connectWebSocket(url: string): Promise<WebSocket> {
  return new Promise((resolve, reject) => {
    const ws = new WebSocket(url);

    ws.onopen = () => resolve(ws);
    ws.onerror = (err) => reject(err);
    ws.onmessage = (ev) => {
      console.log(ev.data)
    }
  });
}

class CustomRunner implements VitestRunner {
  public config: VitestRunnerConfig;
  public pool: string = "Jstz custom pool";
  wsConn: Promise<WebSocket>;

  declare private __vitest_executor: VitestExecutor;

  constructor(config: VitestRunnerConfig) {
    this.config = config;
    console.log(config)
    this.wsConn = connectWebSocket("ws://localhost:54322");
  }

  // async onBeforeCollect(paths: string[]) {
  //   (await this.wsConn).send(`onBeforeCollect ${paths.join(", ")}`);
  // }

  // async onCollectStart(file: File) {
  //   (await this.wsConn).send(
  //     `onCollectStart: File ${file.filepath} contains tasks [${file.tasks.map((t) => t.name).join(", ")}]`,
  //   );
  // }

  async onCollected(files: File[]) {
    (await this.wsConn).send(
      `onCollected ${files.map((file) => file.filepath).join(", ")}`,
    );
  }

  // async cancel(reason: CancelReason) {
  //   (await this.wsConn).send("cancel");
  // }

  // async onBeforeRunTask(test: Test) {
  //   (await this.wsConn).send(`onBeforeRunTask ${test.name}`);
  // }

  // async onBeforeTryTask(
  //   test: Test,
  //   options: {
  //     retry: number;
  //     repeats: number;
  //   },
  // ) {
  //   (await this.wsConn).send(`onBeforeTryTask ${test.name}`);
  // }

  // async onTaskFinished(test: Test) {
  //   (await this.wsConn).send(`onTaskFinished ${test.name}`);
  // }

  // async onAfterRunTask(test: Test) {
  //   (await this.wsConn).send(`onAfterRunTask ${test.name}`);
  // }

  // async onAfterTryTask(
  //   test: Test,
  //   options: {
  //     retry: number;
  //     repeats: number;
  //   },
  // ) {
  //   (await this.wsConn).send(`onAfterTrytask ${test.name}`);
  // }

  // async onBeforeRunSuite(suite: Suite) {
  //   (await this.wsConn).send(
  //     `onBeforeRunSuite ${suite.file.filepath} ${suite.name}`,
  //   );
  // }

  // async onAfterRunSuite(suite: Suite) {
  //   (await this.wsConn).send(
  //     `onAfterRunSuite ${Object.entries(suite)["filepath"]} ${suite.file.filepath} ${suite.name}`,
  //   );
  // }

  // async runSuite(suite: Suite) {


  // }

  async runTask(test: Test) {
    (await this.wsConn).send(`runTask ${test.file.filepath} ${test.name}`);
    await new Promise((resolve) => {
      console.log("Fake waiting to receive test results")
      setTimeout(() => resolve(null), 1000)
    })

    // const result: TaskResult = {
    //   state: "fail",
    //   errors: [
    //     {
    //       message: "Blah blah blah",
    //     },
    //   ],
    // };
    // test.result = result;
  }

  // async onTaskUpdate(task: TaskResultPack[], events: TaskEventPack[]) {
  //   (await this.wsConn).send(
  //     `onTaskUpdate ${task.map((t) => t[0])}`
  //   );
  // }

  // async onTestAnnotate(
  //   test: Test,
  //   annotation: TestAnnotation,
  // ): Promise<TestAnnotation> {
  //   (await this.wsConn).send(`onTestAnnotate ${test.file.filepath}`);
  //   return annotation;
  // }

  // async onBeforeRunFiles(files: File[]) {
  //   (await this.wsConn).send(
  //     `onBeforeRunFiles ${files.map((file) => file.filepath).join(", ")}`
  //   );
  // }

  // async onAfterRunFiles(files: File[]) {
  //   (await this.wsConn).send(
  //     `onAfterRunFiles ${files.map((f) => f.filepath).join(", ")}`,
  //   );
  // }

  /**
   * Called when test and setup files are imported. Can be called in two situations: when collecting tests and when importing setup files.
   */
  async importFile(filepath: string, source: VitestRunnerImportSource) {
    (await this.wsConn).send(`importFile ${filepath}`);
    const result = await this.__vitest_executor.executeId(filepath);
    console.log(result);
  }

  // async injectValue(key: string) {
  //   (await this.wsConn).send("inject value");
  // }

  // getImportDurations(): Record<string, ImportDuration> {
  //     throw new Error("Not implemented")
  // }
}

export default CustomRunner;
