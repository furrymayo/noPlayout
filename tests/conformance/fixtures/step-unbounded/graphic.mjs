// Conformance fixture (M1 task 9): `stepCount: -1`. Plays report steps 0, 1, 2 and then the end
// (`currentStep` undefined, O7); NEXT STEP keeps sending `delta: 1` until then (O6).
const LAST = 2;

class StepUnbounded extends HTMLElement {
  async load(params) {
    if (params.renderType !== "realtime") {
      return { statusCode: 400, statusMessage: "realtime only" };
    }
    this.step = undefined;
    this.ended = false;
    this.label = document.createElement("div");
    this.label.style.cssText =
      "position:absolute;left:64px;top:64px;width:256px;height:64px;background:#2060c0";
    this.label.hidden = true;
    this.appendChild(this.label);
    return { statusCode: 200 };
  }

  async playAction(params) {
    const delta = params.delta ?? 1;
    if (this.ended) {
      return { statusCode: 200, currentStep: undefined };
    }
    const next = (this.step ?? -1) + delta;
    if (next > LAST) {
      this.ended = true;
      this.step = undefined;
      this.label.hidden = true;
      return { statusCode: 200, currentStep: undefined };
    }
    this.step = next;
    this.label.hidden = false;
    this.label.style.width = `${256 + 64 * next}px`;
    return { statusCode: 200, currentStep: this.step };
  }

  async updateAction() {
    return { statusCode: 200 };
  }

  async stopAction() {
    this.label.hidden = true;
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

export default StepUnbounded;
