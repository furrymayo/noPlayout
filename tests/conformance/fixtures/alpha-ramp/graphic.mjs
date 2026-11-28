// Pixel fixture (M1 task 10): 16 white patches of 64 × 64 px at alpha 17·i, i = 0..15, in one row
// from (64, 64) with a 16 px gap, on a transparent background. The pixel test reads each
// patch's centre from the KEY plane and expects 17·i within ±1 LSB (done-when (b)). Static:
// painted once on load, never again.
export const PATCH = 64;
export const GAP = 16;
export const X0 = 64;
export const Y0 = 64;

class AlphaRamp extends HTMLElement {
  async load(params) {
    if (params.renderType !== "realtime") {
      return { statusCode: 400, statusMessage: "realtime only" };
    }
    const c = document.createElement("canvas");
    c.width = window.innerWidth;
    c.height = window.innerHeight;
    c.style.cssText = "position:absolute;left:0;top:0;width:100%;height:100%";
    this.appendChild(c);
    const ctx = c.getContext("2d", { alpha: true });
    for (let i = 0; i < 16; i++) {
      ctx.fillStyle = `rgba(255, 255, 255, ${(17 * i) / 255})`;
      ctx.fillRect(X0 + i * (PATCH + GAP), Y0, PATCH, PATCH);
    }
    return { statusCode: 200 };
  }

  async playAction() {
    return { statusCode: 200, currentStep: 0 };
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

export default AlphaRamp;
