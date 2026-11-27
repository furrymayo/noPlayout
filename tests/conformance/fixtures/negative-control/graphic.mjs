// Test-only conformance fixture (A8): a graphic that misbehaves on purpose so the harness can
// prove it fails. The fault is chosen once, by the load data's `fault`:
//   console-error  console.error during load
//   uncaught       an exception nothing catches, thrown from a timer during load
//   status         every play answers statusCode 500
//   wrong-step     every play reports one step ahead (play #1 says currentStep 1)
// Anything else (and the update's schema default "none") behaves.

class NegativeControl extends HTMLElement {
  async load(params) {
    if (params.renderType !== "realtime") {
      return { statusCode: 400, statusMessage: "realtime only" };
    }
    this.fault = params.data?.fault ?? "none";
    this.step = -1;
    if (this.fault === "console-error") {
      console.error("negative-control: deliberate console.error");
    }
    if (this.fault === "uncaught") {
      setTimeout(() => {
        throw new Error("negative-control: deliberate uncaught exception");
      }, 0);
      await new Promise((resolve) => setTimeout(resolve, 50));
    }
    return { statusCode: 200 };
  }

  async playAction(params) {
    if (this.fault === "status") {
      return { statusCode: 500, statusMessage: "negative-control: deliberate failure" };
    }
    this.step = Math.min(this.step + (params.delta ?? 1), 1);
    const reported = this.fault === "wrong-step" ? this.step + 1 : this.step;
    return { statusCode: 200, currentStep: reported };
  }

  async updateAction() {
    return { statusCode: 200 };
  }

  async stopAction() {
    return { statusCode: 200 };
  }

  async customAction() {
    return { statusCode: 400, statusMessage: "no custom actions" };
  }

  async dispose() {
    this.replaceChildren();
    return { statusCode: 200 };
  }
}

export default NegativeControl;
