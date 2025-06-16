export class NotSupported extends Error {
  constructor(msg) {
    super(msg);
    this.name = "NotSupported";
  }
}
